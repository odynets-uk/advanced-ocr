use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use calamine::{Reader, Xlsx, open_workbook};
use docx_rs::read_docx;

use crate::ocr_engine::OcrEngine;

/// Supported file types
#[derive(Debug, Clone)]
pub enum FileType {
    Image(ImageFormat),
    Pdf,
    Docx,
    Xlsx,
    Xls,
    Archive(ArchiveFormat),
    Unsupported,
}

#[derive(Debug, Clone)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Bmp,
    Tiff,
    Gif,
    Webp,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum ArchiveFormat {
    Zip,
    Tar,
    Rar,
    Unknown,
}

/// Processing result for a single file
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub file_type: FileType,
    pub page_count: usize,
    pub text: String,
}

/// Main file processor
pub struct FileProcessor {
    use_pdf_ocr: bool,
}

impl FileProcessor {
    pub fn new(use_pdf_ocr: bool) -> Self {
        FileProcessor { use_pdf_ocr }
    }

    pub fn process_file(
        &self,
        path: &Path,
        ocr_engine: &OcrEngine,
    ) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        let file_type = FileType::from_path(path);

        match file_type {
            FileType::Image(_) => {
                self.process_image(path, ocr_engine)
            }
            FileType::Pdf => {
                self.process_pdf(path, ocr_engine)
            }
            FileType::Docx => {
                self.process_docx(path)
            }
            FileType::Xlsx | FileType::Xls => {
                self.process_excel(path)
            }
            FileType::Archive(_) => {
                self.process_archive(path, ocr_engine)
            }
            FileType::Unsupported => {
                Err("Unsupported file format".into())
            }
        }
    }

    fn process_image(
        &self,
        path: &Path,
        ocr_engine: &OcrEngine,
    ) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        let text = ocr_engine.extract_text_from_image(path)
            .map_err(|e| format!("OCR error: {}", e))?;

        Ok(vec![ProcessResult {
            file_type: FileType::from_path(path),
            page_count: 1,
            text,
        }])
    }

    fn process_pdf(
        &self,
        path: &Path,
        ocr_engine: &OcrEngine,
    ) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        use pdf::file::FileOptions;

        let pdf_data = fs::read(path)?;

        // Try to extract text directly from PDF
        let direct_text = match pdf_extract::extract_text_from_mem(&pdf_data) {
            Ok(text) if !text.trim().is_empty() && text.len() > 100 => {
                Some(text)
            }
            _ => None,
        };

        let text = if let Some(text) = direct_text {
            text
        } else if self.use_pdf_ocr {
            // Fall back to OCR
            self.extract_text_from_pdf_with_ocr(path, ocr_engine)?
        } else {
            String::new()
        };

        // Get page count
        let page_count = FileOptions::cached()
            .load(&pdf_data)
            .map(|pdf| pdf.num_pages())
            .unwrap_or(1);

        Ok(vec![ProcessResult {
            file_type: FileType::Pdf,
            page_count,
            text,
        }])
    }

    fn extract_text_from_pdf_with_ocr(
        &self,
        path: &Path,
        ocr_engine: &OcrEngine,
    ) -> Result<String, Box<dyn Error>> {
        use pdf::file::FileOptions;

        let pdf_data = fs::read(path)?;
        let pdf = FileOptions::cached().load(&pdf_data)?;
        let mut combined_text = String::new();

        for (i, page) in pdf.pages().enumerate() {
            let page = page?;

            // Try to extract images from PDF page
            if let Some(resources) = page.resources() {
                for (_, xobject) in &resources.xobjects {
                    if let pdf::object::XObject::Image(ref image) = *xobject {
                        if let Ok(image_data) = image.data() {
                            match ocr_engine.extract_text_from_image_data(&image_data) {
                                Ok(text) if !text.trim().is_empty() => {
                                    combined_text.push_str(&format!("\n--- Page {} ---\n", i + 1));
                                    combined_text.push_str(&text);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(combined_text)
    }

    fn process_docx(&self, path: &Path) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        let docx_data = fs::read(path)?;
        let docx = read_docx(&docx_data)
            .map_err(|e| format!("Failed to parse DOCX: {}", e))?;

        // Extract text from paragraphs
        let mut text = String::new();

        if let Some(document) = docx.document {
            for child in &document.children {
                // Extract text from paragraphs
                if let docx_rs::DocumentChild::Paragraph(p) = child {
                    for p_child in &p.children {
                        if let docx_rs::ParagraphChild::Run(r) = p_child {
                            for r_child in &r.children {
                                if let docx_rs::RunChild::Text(t) = r_child {
                                    text.push_str(&t.text);
                                    text.push(' ');
                                }
                            }
                        }
                    }
                    text.push('\n');
                }

                // Extract text from tables
                if let docx_rs::DocumentChild::Table(tbl) = child {
                    for row in &tbl.rows {
                        for cell in &row.cells {
                            for cell_child in &cell.children {
                                if let docx_rs::TableCellContent::Paragraph(p) = cell_child {
                                    for p_child in &p.children {
                                        if let docx_rs::ParagraphChild::Run(r) = p_child {
                                            for r_child in &r.children {
                                                if let docx_rs::RunChild::Text(t) = r_child {
                                                    text.push_str(&t.text);
                                                    text.push(' ');
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            text.push('\t'); // Tab separator for cells
                        }
                        text.push('\n'); // New line for each row
                    }
                }
            }
        }

        // Estimate page count (approximate: 500 words per page)
        let word_count = text.split_whitespace().count();
        let page_count = (word_count as f32 / 500.0).ceil() as usize;

        Ok(vec![ProcessResult {
            file_type: FileType::Docx,
            page_count: page_count.max(1),
            text,
        }])
    }

    fn process_excel(&self, path: &Path) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        let mut workbook: Xlsx<_> = open_workbook(path)?;
        let mut text = String::new();

        // Get sheet names
        let sheet_names = workbook.sheet_names().to_vec();

        for sheet_name in &sheet_names {
            text.push_str(&format!("\n=== Sheet: {} ===\n", sheet_name));

            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                for row in range.rows() {
                    for cell in row.iter() {
                        // Convert cell to string
                        let cell_text = match cell {
                            calamine::DataType::String(s) => s.to_string(),
                            calamine::DataType::Float(f) => f.to_string(),
                            calamine::DataType::Int(i) => i.to_string(),
                            calamine::DataType::Bool(b) => b.to_string(),
                            calamine::DataType::DateTime(dt) => dt.to_string(),
                            calamine::DataType::Duration(d) => d.to_string(),
                            calamine::DataType::Time(t) => t.to_string(),
                            calamine::DataType::Error(e) => format!("[Error: {:?}]", e),
                            _ => String::new(),
                        };

                        text.push_str(&cell_text);
                        text.push('\t'); // Tab separator
                    }
                    text.push('\n'); // New line for each row
                }
            }
        }

        Ok(vec![ProcessResult {
            file_type: FileType::from_path(path),
            page_count: sheet_names.len().max(1),
            text,
        }])
    }

    fn process_archive(
        &self,
        _path: &Path,
        _ocr_engine: &OcrEngine,
    ) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        // Placeholder for archive processing
        // Would extract and process contained files
        Err("Archive processing not implemented in this version".into())
    }
}

impl FileType {
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => {
                let ext_lower = ext.to_lowercase();
                match ext_lower.as_str() {
                    "jpg" | "jpeg" => FileType::Image(ImageFormat::Jpeg),
                    "png" => FileType::Image(ImageFormat::Png),
                    "bmp" => FileType::Image(ImageFormat::Bmp),
                    "tiff" | "tif" => FileType::Image(ImageFormat::Tiff),
                    "gif" => FileType::Image(ImageFormat::Gif),
                    "webp" => FileType::Image(ImageFormat::Webp),
                    "pdf" => FileType::Pdf,
                    "docx" => FileType::Docx,
                    "xlsx" => FileType::Xlsx,
                    "xls" => FileType::Xls,
                    "zip" => FileType::Archive(ArchiveFormat::Zip),
                    "tar" => FileType::Archive(ArchiveFormat::Tar),
                    "rar" => FileType::Archive(ArchiveFormat::Rar),
                    _ => FileType::Unsupported,
                }
            }
            None => FileType::Unsupported,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            FileType::Image(format) => format!("Image ({:?})", format),
            FileType::Pdf => "PDF".to_string(),
            FileType::Docx => "DOCX".to_string(),
            FileType::Xlsx => "XLSX".to_string(),
            FileType::Xls => "XLS".to_string(),
            FileType::Archive(format) => format!("Archive ({:?})", format),
            FileType::Unsupported => "Unsupported".to_string(),
        }
    }
}
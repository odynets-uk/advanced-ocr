use std::error::Error;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum PdfCreationMethod {
    /// Use ocrmypdf (Python) - best quality, requires installation
    #[value(name = "ocrmypdf")]
    OcrMyPdf,
    /// Use native Rust (pdf_writer) - basic fallback, image-only PDF
    Native,
}


pub fn check_ocrmypdf_installed() -> bool {
    which::which("ocrmypdf").is_ok()
}

pub fn create_searchable_pdf(
    image_path: &Path,
    ocr_text: &str,
    output_path: &Path,
    language: &str,
    method: PdfCreationMethod,
) -> Result<(), Box<dyn Error>> {
    match method {
        PdfCreationMethod::OcrMyPdf => {
            create_with_ocrmypdf(image_path, output_path, language)
        }
        PdfCreationMethod::Native => {
            create_with_pdf_writer(image_path, ocr_text, output_path)
        }
    }
}

pub fn create_with_ocrmypdf(
    image_path: &Path,
    output_path: &Path,
    language: &str,
) -> Result<(), Box<dyn Error>> {
    if !check_ocrmypdf_installed() {
        return Err("ocrmypdf is not installed!\n\n\
            Installation instructions:\n\
            • Windows: pip install ocrmypdf\n\
            • Linux: sudo apt install ocrmypdf\n\
            • macOS: brew install ocrmypdf\n\n\
            Or use --pdf-method native for Rust-based PDF creation\n\
            More info: https://ocrmypdf.readthedocs.io/en/latest/installation.html".to_string().into());
    }

    let temp_dir = std::env::temp_dir();
    let temp_rgb = temp_dir.join(format!(
        "ocr_temp_{}.png",
        image_path.file_stem().unwrap().to_string_lossy()
    ));

    // Convert to RGB PNG
    let img = image::open(image_path)?;
    let rgb_img = img.to_rgb8();
    image::save_buffer(
        &temp_rgb,
        &rgb_img,
        rgb_img.width(),
        rgb_img.height(),
        image::ColorType::Rgb8,
    )?;

    let output = Command::new("ocrmypdf")
        .arg("-l")
        .arg(language)
        .arg("--image-dpi")
        .arg("300")
        .arg(&temp_rgb)
        .arg(output_path)
        .stderr(std::process::Stdio::piped())
        .output()?;

    let _ = std::fs::remove_file(&temp_rgb);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ocrmypdf failed: {}", stderr).into());
    }

    Ok(())
}

pub fn create_with_pdf_writer(
    image_path: &Path,
    ocr_text: &str,
    output_path: &Path,
) -> Result<(), Box<dyn Error>> {
    use pdf_writer::{Pdf, Rect, Content, Str, Name, Ref, Finish, Filter};
    use pdf_writer::types::TextRenderingMode;
    use image::GenericImageView;

    let img = image::open(image_path)?;
    let (width, height) = img.dimensions();

    // Convert to JPEG
    let temp_dir = std::env::temp_dir();
    let temp_jpg = temp_dir.join("temp_ocr.jpg");
    img.save(&temp_jpg)?;
    let img_data = std::fs::read(&temp_jpg)?;
    let _ = std::fs::remove_file(&temp_jpg);

    let mut pdf = Pdf::new();

    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let page_id = Ref::new(3);
    let image_id = Ref::new(4);
    let content_id = Ref::new(5);
    let font_id = Ref::new(6);

    // Catalog
    pdf.catalog(catalog_id).pages(page_tree_id);

    // Page tree
    pdf.pages(page_tree_id).kids([page_id]).count(1);

    // Page
    let mut page = pdf.page(page_id);
    page.parent(page_tree_id);
    page.media_box(Rect::new(0.0, 0.0, width as f32, height as f32));
    page.contents(content_id);

    let mut resources = page.resources();
    resources.x_objects().pair(Name(b"Im1"), image_id);
    resources.fonts().pair(Name(b"F1"), font_id);
    resources.finish();
    page.finish();

    // Font
    pdf.type1_font(font_id).base_font(Name(b"Helvetica"));

    // Image
    let mut image = pdf.image_xobject(image_id, &img_data);
    image.width(width as i32);
    image.height(height as i32);
    image.color_space().device_rgb();
    image.bits_per_component(8);
    image.filter(Filter::DctDecode).finish();  // ✅ Додати .finish()
    image.finish();

    // Content: image + invisible text
    let mut content = Content::new();

    // Draw image
    content.save_state();
    content.transform([width as f32, 0.0, 0.0, height as f32, 0.0, 0.0]);
    content.x_object(Name(b"Im1"));
    content.restore_state();

    // Add invisible text
    content.begin_text();
    content.set_text_rendering_mode(TextRenderingMode::Invisible);  // ✅ Правильний enum
    content.set_font(Name(b"F1"), 12.0);
    content.next_line(10.0, height as f32 - 20.0);

    let escaped = ocr_text
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)");
    content.show(Str(escaped.as_bytes()));  // ✅ Str() замість TextStr()

    content.end_text();

    pdf.stream(content_id, &content.finish());

    // Write to file
    std::fs::write(output_path, pdf.finish())?;

    Ok(())
}




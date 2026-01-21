use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use image::GenericImageView;

use crate::file_processors::FileType;
use crate::OcrResult;

/// Setup input and output directories
#[allow(dead_code)]
pub fn setup_directories(input_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    if !input_dir.exists() {
        fs::create_dir_all(input_dir)?;
        println!("Created 'input' directory. Please add your files there.");
        println!("Supported formats:");
        println!("  - Images: jpg, jpeg, png, bmp, tiff, gif, webp");
        println!("  - Documents: pdf, docx, xlsx, xls");
        return Err("Input directory was empty".into());
    }

    fs::create_dir_all(output_dir)?;
    Ok(())
}

/// Extract metadata from file
pub fn extract_metadata(file_path: &Path, file_type: &FileType) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    metadata.insert("path".to_string(), file_path.display().to_string());

    if let Ok(file_meta) = file_path.metadata() {
        metadata.insert("size".to_string(), format!("{} bytes", file_meta.len()));
        if let Ok(modified) = file_meta.modified() {
            metadata.insert("modified".to_string(), format!("{:?}", modified));
        }
    }

    // Add type-specific metadata
    match file_type {
        FileType::Image(_) => {
            if let Ok(img) = image::open(file_path) {
                let (width, height) = GenericImageView::dimensions(&img);
                metadata.insert("dimensions".to_string(), format!("{}x{}", width, height));
                metadata.insert("color_type".to_string(), format!("{:?}", img.color()));
            }
        }
        FileType::Pdf => {
            // PDF metadata would go here
            metadata.insert("type".to_string(), "PDF Document".to_string());
        }
        FileType::Docx => {
            metadata.insert("type".to_string(), "Word Document".to_string());
        }
        FileType::Xlsx | FileType::Xls => {
            metadata.insert("type".to_string(), "Excel Spreadsheet".to_string());
        }
        _ => {}
    }

    metadata
}

/// Save processing results to disk
/// Save processing results to disk
pub fn save_results(
    results: &[OcrResult],
    output_dir: &Path,
    save_individual_files: bool,
) -> Result<(), Box<dyn Error>> {
    // Save to CSV (without metadata field)
    let csv_path = output_dir.join("results.csv");
    let mut wtr = csv::Writer::from_path(&csv_path)?;

    // Write header manually
    wtr.write_record(&["filename", "file_type", "page_count", "text_length", "processing_time_ms", "error"])?;

    for result in results {
        wtr.write_record(&[
            &result.filename,
            &result.file_type,
            &result.page_count.to_string(),
            &result.text.len().to_string(),  // ✅ Змінено: довжина замість повного тексту
            &result.processing_time_ms.to_string(),
            &result.error.as_ref().unwrap_or(&String::new()),
        ])?;
    }
    wtr.flush()?;
    log::info!("Results saved to: {}", csv_path.display());

    if save_individual_files {
        // Save individual text files
        let texts_dir = output_dir.join("texts");
        fs::create_dir_all(&texts_dir)?;

        for result in results {
            if result.error.is_none() && !result.text.is_empty() {
                let base_name = Path::new(&result.filename)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                let text_path = texts_dir.join(format!("{}.txt", base_name));
                fs::write(&text_path, &result.text)?;
            }
        }

        log::info!("Text files saved to: {}", texts_dir.display());
    }

    // Save full metadata as JSON (включаючи metadata)
    let json_path = output_dir.join("metadata.json");
    let json_data = serde_json::to_string_pretty(&results)?;
    fs::write(&json_path, json_data)?;
    log::info!("Metadata saved to: {}", json_path.display());

    Ok(())
}

/// Generate a detailed report
pub fn generate_report(results: &[OcrResult], output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let report_path = output_dir.join("report.txt");
    let mut report = String::new();

    let total_files = results.len();
    let successful: Vec<&OcrResult> = results.iter().filter(|r| r.error.is_none()).collect();
    let failed: Vec<&OcrResult> = results.iter().filter(|r| r.error.is_some()).collect();
    let total_pages: usize = results.iter().map(|r| r.page_count).sum();
    let total_text_chars: usize = results.iter().map(|r| r.text.chars().count()).sum();

    report.push_str("=== OCR Processing Report ===\n\n");
    report.push_str("Overall Statistics:\n");
    report.push_str(&format!("  - Files processed: {}\n", total_files));
    report.push_str(&format!("  - Successful: {} ({:.1}%)\n",
                             successful.len(),
                             (successful.len() as f32 / total_files as f32) * 100.0));
    report.push_str(&format!("  - Failed: {} ({:.1}%)\n",
                             failed.len(),
                             (failed.len() as f32 / total_files as f32) * 100.0));
    report.push_str(&format!("  - Total pages: {}\n", total_pages));
    report.push_str(&format!("  - Total text characters: {}\n\n", total_text_chars));

    // Distribution by file type
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut type_chars: HashMap<String, usize> = HashMap::new();

    for result in &successful {
        *type_counts.entry(result.file_type.clone()).or_insert(0) += 1;
        *type_chars.entry(result.file_type.clone()).or_insert(0) += result.text.chars().count();
    }

    report.push_str("Distribution by file type (successful only):\n");
    for (file_type, count) in &type_counts {
        let avg_chars = type_chars.get(file_type).unwrap_or(&0) / count.max(&1);
        report.push_str(&format!("  - {}: {} files, avg {} chars/file\n",
                                 file_type, count, avg_chars));
    }
    report.push_str("\n");

    // List of failed files
    if !failed.is_empty() {
        report.push_str("Failed files:\n");
        for result in failed {
            report.push_str(&format!("  - {}: {}\n",
                                     result.filename,
                                     result.error.as_ref().unwrap()));
        }
        report.push_str("\n");
    }

    // Top files by text size
    let mut sorted_by_size = successful.clone();
    sorted_by_size.sort_by(|a, b| b.text.len().cmp(&a.text.len()));

    if !sorted_by_size.is_empty() {
        report.push_str("Top 5 files by text size:\n");
        for (i, result) in sorted_by_size.iter().take(5).enumerate() {
            report.push_str(&format!("  {}. {}: {} characters, {} pages\n",
                                     i + 1,
                                     result.filename,
                                     result.text.chars().count(),
                                     result.page_count));
        }
    }

    fs::write(&report_path, report)?;
    log::info!("Report saved to: {}", report_path.display());

    Ok(())
}
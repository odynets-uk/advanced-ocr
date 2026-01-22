use clap::Parser;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use walkdir::WalkDir;



mod file_processors;
mod ocr_engine;
mod utils;

use crate::file_processors::{FileProcessor, FileType};
use crate::ocr_engine::OcrEngine;
use crate::utils::{extract_metadata, generate_report, save_results};

#[derive(Debug, Clone, serde::Serialize)]
struct OcrResult {
    filename: String,
    file_type: String,
    page_count: usize,
    text: String,
    processing_time_ms: u128,
    error: Option<String>,
    metadata: HashMap<String, String>,
}

/// Advanced Batch OCR in Rust
#[derive(Parser, Debug)]
#[command(name = "Advanced OCR")]
#[command(about = "Batch OCR for PDF, DOCX, XLSX, and images", long_about = None)]
struct Cli {
    /// Input directory path
    #[arg(short, long, default_value = "./input")]
    input: PathBuf,

    /// Output directory path
    #[arg(short, long, default_value = "./output")]
    output: PathBuf,

    /// OCR languages (comma-separated: ukr,eng)
    #[arg(short, long, default_value = "ukr+eng")]
    languages: String,

    /// Enable OCR for PDF images (slower)
    #[arg(long, default_value = "false")]
    pdf_ocr: bool,

    /// Number of parallel workers
    #[arg(short, long, default_value = "4")]
    workers: usize,

    /// Save individual text files
    #[arg(long, default_value = "true")]
    save_texts: bool,

    /// Create searchable PDFs from images
    #[arg(long)]
    searchable_pdf: bool,

    /// DPI for OCR (default: 300)
    #[arg(long, default_value = "300")]
    dpi: u32,

    /// Page segmentation mode (default: 3)
    #[arg(long, default_value = "3")]
    psm: u8,

    /// OCR Engine Mode (default: 3)
    #[arg(long, default_value = "3")]
    oem: u8,
}

fn collect_files(input_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(input_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let file_type = FileType::from_path(path);
            if !matches!(file_type, FileType::Unsupported) {
                files.push(path.to_path_buf());
            }
        }
    }

    files
}

fn process_single_file(
    path: PathBuf,
    ocr_engine: &OcrEngine,
    file_processor: &FileProcessor,
    pb: &ProgressBar,
) -> Vec<OcrResult> {
    let start = Instant::now();
    let filename = path.file_name().unwrap().to_string_lossy().to_string();

    pb.set_message(format!("Processing: {}", filename));

    let mut results = Vec::new();

    match file_processor.process_file(&path, ocr_engine) {
        Ok(process_results) => {
            for result in process_results {
                let processing_time = start.elapsed().as_millis();
                let metadata = extract_metadata(&path, &result.file_type);

                results.push(OcrResult {
                    filename: filename.clone(),
                    file_type: result.file_type.to_string(),
                    page_count: result.page_count,
                    text: result.text,
                    processing_time_ms: processing_time,
                    error: None,
                    metadata,
                });
            }
        }
        Err(e) => {
            let processing_time = start.elapsed().as_millis();
            let file_type = FileType::from_path(&path);

            results.push(OcrResult {
                filename,
                file_type: file_type.to_string(),
                page_count: 0,
                text: String::new(),
                processing_time_ms: processing_time,
                error: Some(format!("Processing error: {}", e)),
                metadata: HashMap::new(),
            });
        }
    }

    pb.inc(1);
    results
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    env_logger::init();

    println!("=== Advanced Batch OCR in Rust ===");
    println!("Supports: PDF, DOCX, XLSX, JPG, PNG, BMP, TIFF, GIF, WebP");

    // Create directories if they don't exist
    std::fs::create_dir_all(&cli.input)?;
    std::fs::create_dir_all(&cli.output)?;

    println!("\nSupported formats:");
    println!("  - Images: jpg, jpeg, png, bmp, tiff, gif, webp");
    println!("  - Documents: pdf, docx, xlsx, xls");

    // Collect files
    let files = collect_files(&cli.input);

    if files.is_empty() {
        return Err("Input directory was empty".into());
    }

    println!("\nFound {} files to process", files.len());
    println!("OCR Language: {}", cli.languages);
    println!("PDF OCR: {}", if cli.pdf_ocr { "enabled" } else { "disabled" });
    println!("Workers: {}", cli.workers);

    // Initialize OCR engine
    let ocr_engine = OcrEngine::new(&cli.languages)?;

    // Initialize file processor
    let processor = FileProcessor::new(cli.pdf_ocr);

    // Setup thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(cli.workers)
        .build()?;

    // Setup progress bars
    let start_time = std::time::Instant::now();
    let main_pb = ProgressBar::new(files.len() as u64);
    main_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")?
            .progress_chars("#>-"),
    );

    let processed = Arc::new(AtomicUsize::new(0));

    // Cloning for Threads
    let pb_clone = main_pb.clone();
    let processed_clone = Arc::clone(&processed);

    let results: Vec<OcrResult> = pool.install(|| {
        files
            .par_iter()
            .map(|file| {
                let file_name = file.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Обробка файлу
                let result = process_single_file(file.clone(), &ocr_engine, &processor, &pb_clone);

                // Atomic increment
                let count = processed_clone.fetch_add(1, Ordering::SeqCst) + 1;
                pb_clone.set_position(count as u64);
                pb_clone.set_message(format!("Processed: {}", file_name));

                result
            })
            .flatten()
            .collect()
    });


    main_pb.finish_with_message("Processing complete!");

    // Save results and generate report
    save_results(&results, &cli.output, cli.save_texts)?;
    generate_report(&results, &cli.output)?;

    // Display final statistics
    let successful: Vec<&OcrResult> = results.iter().filter(|r| r.error.is_none()).collect();

    let total_time = start_time.elapsed();
    println!("\n=== Processing Complete ===");
    println!("Total time: {:.2} seconds", total_time.as_secs_f64());
    println!("Files processed: {}", results.len());
    println!(
        "Successful: {} ({:.1}%)",
        successful.len(),
        (successful.len() as f32 / results.len() as f32) * 100.0
    );
    println!("Results saved to: {}", cli.output.display());

    if cli.searchable_pdf {
        println!("\nCreating searchable PDFs...");
        let pdf_output = cli.output.join("searchable_pdfs");
        std::fs::create_dir_all(&pdf_output)?;
        let temp_dir = std::env::temp_dir();

        for (file, result) in files.iter().zip(results.iter()) {
            if matches!(FileType::from_path(file), FileType::Image(_)) && result.error.is_none() {
                let output_name = file.file_stem().unwrap().to_string_lossy();
                let output_pdf = pdf_output.join(format!("{}.pdf", output_name));

                let temp_rgb = temp_dir.join(format!("{}_rgb.png", output_name));

                match image::open(file) {
                    Ok(img) => {
                        // Конвертувати в RGB і зберегти як PNG
                        let rgb_img = img.to_rgb8();
                        if let Err(e) = image::save_buffer(
                            &temp_rgb,
                            &rgb_img,
                            rgb_img.width(),
                            rgb_img.height(),
                            image::ColorType::Rgb8,
                        ) {
                            eprintln!("  ✗ Failed to convert {}: {}", output_name, e);
                            continue;
                        }
                    }
                    Err(e) => {
                        eprintln!("  ✗ Failed to open {}: {}", output_name, e);
                        continue;
                    }
                }

                let output = std::process::Command::new("ocrmypdf")
                    .arg("-l")
                    .arg(&cli.languages)
                    .arg("--image-dpi")
                    .arg("300")
                    .arg(&temp_rgb)
                    .arg(&output_pdf)
                    .stderr(std::process::Stdio::piped())
                    .output();

                // Видалити тимчасовий файл
                let _ = std::fs::remove_file(&temp_rgb);

                match output {
                    Ok(o) if o.status.success() => {
                        println!("  ✓ Created: {}", output_pdf.display());
                    }
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        let error_line = stderr.lines()
                            .rfind(|l| l.contains("Error:"))
                            .unwrap_or("Unknown error");
                        eprintln!("  ✗ Failed {}: {}", output_name, error_line);
                    }
                    Err(e) => {
                        eprintln!("  ✗ Error running ocrmypdf for {}: {}", output_name, e);
                    }
                }
            }
        }

        println!("\nSearchable PDFs saved to: {}", pdf_output.display());
    }

    Ok(())
}

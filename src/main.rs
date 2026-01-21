use clap::Parser;
use std::collections::HashMap;
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
        .build()
        .unwrap();

    // Setup progress bars
    let mp = MultiProgress::new();
    let main_pb = mp.add(ProgressBar::new(files.len() as u64));
    main_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Process files in parallel
    let start_time = Instant::now();

    let results: Vec<OcrResult> = pool.install(|| {
        files
            .par_iter()
            .flat_map(|file| {
                process_single_file(file.clone(), &ocr_engine, &processor, &main_pb)
            })
            .collect()
    });

    main_pb.finish_with_message("Processing complete!");

    // Save results and generate report
    save_results(&results, &cli.output, cli.save_texts)?;
    generate_report(&results, &cli.output)?;

    // Display final statistics
    let total_time = start_time.elapsed();
    let successful: Vec<&OcrResult> = results.iter().filter(|r| r.error.is_none()).collect();

    println!("\n=== Processing Complete ===");
    println!("Total time: {:.2} seconds", total_time.as_secs_f32());
    println!("Files processed: {}", results.len());
    println!(
        "Successful: {} ({:.1}%)",
        successful.len(),
        (successful.len() as f32 / results.len() as f32) * 100.0
    );
    println!("Results saved to: {}", cli.output.display());

    if cli.searchable_pdf {
        println!("Creating searchable PDFs...");

        let pdf_output = cli.output.join("searchable_pdfs");
        std::fs::create_dir_all(&pdf_output)?;

        for file in &files {
            if matches!(FileType::from_path(file), FileType::Image(_)) {
                let output_name = file.file_stem().unwrap().to_string_lossy();
                let output_pdf = pdf_output.join(format!("{}.pdf", output_name));

                let status = std::process::Command::new("tesseract")
                    .arg(file)
                    .arg(output_pdf.with_extension(""))
                    .arg("-l")
                    .arg(&cli.languages)
                    .arg("pdf")
                    .status()?;

                if status.success() {
                    println!(" ✓ Created: {}", output_pdf.display());
                }
            }
        }
    }

    Ok(())
}

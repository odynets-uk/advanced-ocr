use clap::Parser;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use walkdir::WalkDir;

mod file_processors;
mod ocr_engine;
mod utils;
mod pdf_creator;

use crate::file_processors::{FileProcessor, FileType};
use crate::ocr_engine::OcrEngine;
use crate::pdf_creator::{create_searchable_pdf, PdfCreationMethod};
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

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PdfMethod {
    /// Use ocrmypdf (Python) - best quality, requires installation
    Ocrmypdf,
    /// Use native Rust (lopdf) - fast, no dependencies
    Native,
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1)
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
    #[arg(short, long, default_value_t = default_workers())]
    workers: usize,

    /// Save individual text files
    #[arg(long, default_value = "true")]
    save_texts: bool,

    /// Create searchable PDFs from images
    #[arg(long)]
    searchable_pdf: bool,

    /// PDF creation method
    #[arg(long, value_enum, default_value = "ocrmypdf")]
    pdf_method: PdfMethod,

    /// DPI for OCR (default: 300)
    #[arg(long, default_value = "300")]
    dpi: String,

    /// Page segmentation mode (default: 3)
    #[arg(long, default_value = "3")]
    psm: u8,

    /// OCR Engine Mode (default: 3)
    #[arg(long, default_value = "3")]
    oem: u8,

    /// Show detailed Tesseract commands and debug output
    #[arg(long, short = 'v')]
    verbose: bool,
}

fn parse_dpi(dpi_arg: &str) -> u32 {
    match dpi_arg.to_lowercase().as_str() {
        "screen" => {
            #[cfg(target_os = "windows")]
            {
                96
            }
            #[cfg(not(target_os = "windows"))]
            {
                72  // macOS, Linux
            }
        }
        _ => {
            dpi_arg.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("⚠️  Invalid DPI value '{}', using default 300", dpi_arg);
                300
            })
        }
    }
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
    pb: &Arc<Mutex<ProgressBar>>,
) -> Vec<OcrResult> {
    let start = Instant::now();
    let filename = path.file_name().unwrap().to_string_lossy().to_string();

    // Lock для set_message
    {
        let pb_guard = pb.lock().unwrap();
        pb_guard.set_message(format!("Processing: {}", filename));
    }

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

    //Lock для inc
    {
        let pb_guard = pb.lock().unwrap();
        pb_guard.inc(1);
    }

    results
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    env_logger::init();

    // Install once at the beginning (safe), disable debug output if not verbose
    if !cli.verbose {
        unsafe {
            std::env::set_var("TESSERACT_QUIET", "1");
        }
    }

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

    let dpi = parse_dpi(&cli.dpi);

    #[cfg(target_os = "windows")]
    let platform = "Windows";
    #[cfg(target_os = "macos")]
    let platform = "macOS";
    #[cfg(target_os = "linux")]
    let platform = "Linux";

    if cli.dpi.to_lowercase() == "screen" {
        println!("Using screen DPI for {}: {}", platform, dpi);
    }

    // parsing dpi
    let dpi = parse_dpi(&cli.dpi);

    println!("\nFound {} files to process", files.len());
    println!("OCR Language: {}", cli.languages);
    println!("OCR DPI: {}", dpi);
    println!("PDF OCR: {}", if cli.pdf_ocr { "enabled" } else { "disabled" });

    // Initialize OCR engine
    let ocr_engine = OcrEngine::with_config(&cli.languages, dpi, cli.psm, cli.oem, cli.verbose)?;

    // Initialize file processor
    let processor = FileProcessor::new(cli.pdf_ocr);

    // Determine optimal worker count
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    // Scale workers based on file count, but cap at cli.workers
    let worker_count = files.len().min(cli.workers);

    // Setup thread pool with actual worker count
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build()?;

    println!("Workers: {} (of {} CPU cores, {} files)", worker_count, cpu_count, files.len());

    // Setup progress bars
    let start_time = std::time::Instant::now();
    let main_pb = Arc::new(Mutex::new(ProgressBar::new(files.len() as u64)));
    {
        let pb = main_pb.lock().unwrap();
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")?
                .progress_chars("#>-"),
        );
    }

    let results: Vec<OcrResult> = pool.install(|| {
        files
            .par_iter()
            .map(|file| {
                process_single_file(file.clone(), &ocr_engine, &processor, &main_pb)
            })
            .flatten()
            .collect()
    });


    // Finish also via lock
    {
        let pb = main_pb.lock().unwrap();
        pb.finish_with_message("Processing complete!");
    }

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
        // ✅ Визначити який метод використовувати
        let (method, method_name) = match cli.pdf_method {
            PdfMethod::Ocrmypdf => {
                if pdf_creator::check_ocrmypdf_installed() {
                    (PdfCreationMethod::OcrMyPdf, "ocrmypdf")
                } else {
                    eprintln!("\n⚠️  ocrmypdf is not installed, falling back to native Rust method");
                    eprintln!("\n📦 To use ocrmypdf (recommended for searchable PDFs):");
                    eprintln!("  • Windows: pip install ocrmypdf");
                    eprintln!("  • Linux: sudo apt install ocrmypdf");
                    eprintln!("  • macOS: brew install ocrmypdf");
                    eprintln!("  📚 More info: https://ocrmypdf.readthedocs.io/en/latest/installation.html");
                    eprintln!("\n⚠️  Native method will create image-only PDFs (text not searchable)\n");
                    (PdfCreationMethod::Native, "native (image-only)")
                }
            }
            PdfMethod::Native => {
                eprintln!("\n⚠️  Using native Rust method - PDFs will contain images only (not searchable)");
                eprintln!("💡 For searchable PDFs, use --pdf-method ocrmypdf\n");
                (PdfCreationMethod::Native, "native (image-only)")
            }
        };

        println!("🔍 Creating PDFs using: {}", method_name);
        let pdf_output = cli.output.join("searchable_pdfs");
        std::fs::create_dir_all(&pdf_output)?;

        for (file, result) in files.iter().zip(results.iter()) {
            if matches!(FileType::from_path(file), FileType::Image(_)) && result.error.is_none() {
                let output_name = file.file_stem().unwrap().to_string_lossy();
                let output_pdf = pdf_output.join(format!("{}.pdf", output_name));

                match create_searchable_pdf(file, &result.text, &output_pdf, &cli.languages, method) {
                    Ok(_) => println!("  ✓ {}", output_name),
                    Err(e) => eprintln!("  ✗ {}: {}", output_name, e),
                }
            }
        }

        println!("\n✅ PDFs saved to: {}", pdf_output.display());
    }

    Ok(())
}

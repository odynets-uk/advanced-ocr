use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use cached::proc_macro::cached;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{error, info, warn};
use rayon::prelude::*;
use walkdir::WalkDir;

// Import local modules
mod file_processors;
mod ocr_engine;
mod utils;

use crate::file_processors::{FileProcessor, FileType, ProcessResult};
use crate::ocr_engine::OcrEngine;
use crate::utils::{extract_metadata, generate_report, save_results, setup_directories};

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
    // Initialize logger
    env_logger::init();

    println!("=== Advanced Batch OCR in Rust ===");
    println!("Supports: PDF, DOCX, XLSX, JPG, PNG, BMP, TIFF, GIF, WebP");

    // Configuration
    let input_dir = Path::new("./input");
    let output_dir = Path::new("./output");
    let lang = "ukr+eng"; // OCR languages
    let use_pdf_ocr = true;
    let save_individual_files = true;
    let max_workers = 4;

    // Setup directories
    setup_directories(input_dir, output_dir)?;

    // Initialize OCR engine
    let ocr_engine = OcrEngine::new(lang)?;

    // Initialize file processor
    let file_processor = FileProcessor::new(use_pdf_ocr);

    // Collect files to process
    let files_to_process = collect_files(input_dir);

    if files_to_process.is_empty() {
        println!("No supported files found in 'input' directory.");
        return Ok(());
    }

    println!("Found {} files to process.", files_to_process.len());

    // Setup thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(max_workers)
        .build()
        .unwrap();

    // Setup progress bars
    let mp = MultiProgress::new();
    let main_pb = mp.add(ProgressBar::new(files_to_process.len() as u64));
    main_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Process files in parallel
    let start_time = Instant::now();

    let results: Vec<OcrResult> = pool.install(|| {
        files_to_process
            .par_iter()
            .flat_map(|file| {
                process_single_file(file.clone(), &ocr_engine, &file_processor, &main_pb)
            })
            .collect()
    });

    main_pb.finish_with_message("Processing complete!");

    // Save results and generate report
    save_results(&results, output_dir, save_individual_files)?;
    generate_report(&results, output_dir)?;

    // Display final statistics
    let total_time = start_time.elapsed();
    let successful: Vec<&OcrResult> = results.iter().filter(|r| r.error.is_none()).collect();

    println!("\n=== Processing Complete ===");
    println!("Total time: {:.2} seconds", total_time.as_secs_f32());
    println!("Files processed: {}", results.len());
    println!("Successful: {} ({:.1}%)",
             successful.len(),
             (successful.len() as f32 / results.len() as f32) * 100.0);
    println!("Results saved to: {}", output_dir.display());

    Ok(())
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
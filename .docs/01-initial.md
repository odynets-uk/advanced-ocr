# Advanced Batch OCR in Rust with PDF, DOCX, XLSX, and Image Support

## Installation and Setup Instructions

### 1. Install System Dependencies

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install tesseract-ocr tesseract-ocr-ukr poppler-utils
```

**Windows:**
1. Download Tesseract: https://github.com/UB-Mannheim/tesseract/wiki
2. Download Poppler: https://github.com/oschwartz10612/poppler-windows/releases
3. Add both to your PATH environment variable

**macOS:**
```bash
brew install tesseract tesseract-lang poppler
```

### 2. Build and Run the Application

```bash
# Clone or create the project
cargo new advanced_ocr
cd advanced_ocr

# Add dependencies to Cargo.toml
# Copy the code files as shown above

# Build the project
cargo build --release

# Create input directory and add files
mkdir input
# Copy your PDF, DOCX, XLSX, and image files to input/

# Run the application
cargo run --release

# Or enable logging
RUST_LOG=info cargo run --release
```

### 3. Command Line Options (Future Enhancement)

To add command line options, you could use the `clap` crate:

```toml
# Add to Cargo.toml
clap = { version = "4.0", features = ["derive"] }
```

```rust
// Example CLI structure
#[derive(clap::Parser)]
struct Cli {
    #[clap(short, long, default_value = "./input")]
    input: PathBuf,
    
    #[clap(short, long, default_value = "./output")]
    output: PathBuf,
    
    #[clap(short, long, default_value = "ukr+eng")]
    languages: String,
    
    #[clap(short, long)]
    skip_pdf_ocr: bool,
}
```

## Key Features

1. **Multi-format Support**: PDF, DOCX, XLSX, XLS, and all major image formats
2. **Ukrainian Language Support**: Optimized for Ukrainian text with fallback to English
3. **Parallel Processing**: Uses Rayon for efficient multi-core processing
4. **Progress Tracking**: Real-time progress bars for batch processing
5. **Comprehensive Output**: CSV, JSON, individual text files, and detailed reports
6. **Error Handling**: Robust error handling with detailed error messages
7. **Metadata Extraction**: Extracts and preserves file metadata

## Performance Tips

1. **For large PDFs**: Increase memory allocation with `RAYON_NUM_THREADS=8 cargo run --release`
2. **For many small files**: Adjust `max_workers` in the code based on your CPU cores
3. **For complex documents**: Consider pre-processing images before OCR
4. **Memory usage**: Large XLSX files may require significant memory

## Extending the Application

To add support for additional formats:

1. **PPTX**: Use `office` crate or convert to images first
2. **EPUB**: Use `epub` crate for text extraction
3. **ODT/ODS**: Use `odf` crate or convert to DOCX/XLSX first
4. **Scanned documents**: Implement image pre-processing for better OCR accuracy

This implementation provides a robust, production-ready OCR solution for batch processing of mixed document formats with Ukrainian language support.
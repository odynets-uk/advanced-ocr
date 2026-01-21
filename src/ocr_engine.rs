use std::error::Error;
use tesseract::{Tesseract, TesseractError};

/// OCR Engine wrapper for Tesseract
pub struct OcrEngine {
    language: String,
}

impl OcrEngine {
    pub fn new(language: &str) -> Result<Self, Box<dyn Error>> {
        // Verify Tesseract is available
        if Tesseract::new(None, Some(language)).is_err() {
            return Err("Tesseract not found. Please install Tesseract OCR.".into());
        }

        Ok(OcrEngine {
            language: language.to_string(),
        })
    }

    pub fn extract_text_from_image(&self, image_path: &std::path::Path) -> Result<String, Box<dyn Error>> {
        let mut tesseract = Tesseract::new(None, Some(&self.language))?
            .set_image(&image_path.to_string_lossy())?
            .set_variable("preserve_interword_spaces", "1")?;

        Ok(tesseract.get_text()?)
    }

    pub fn extract_text_from_image_data(&self, image_data: &[u8]) -> Result<String, Box<dyn Error>> {
        let mut tesseract = Tesseract::new(None, Some(&self.language))?
            .set_image_from_mem(image_data)?
            .set_variable("preserve_interword_spaces", "1")?;

        Ok(tesseract.get_text()?)
    }
}
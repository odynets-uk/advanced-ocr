use std::error::Error;
use std::io::Write;
use rusty_tesseract::{Args, Image};

pub struct OcrEngine {
    language: String,
}

impl OcrEngine {
    pub fn new(language: &str) -> Result<Self, Box<dyn Error>> {
        Ok(OcrEngine {
            language: language.to_string(),
        })
    }

    pub fn extract_text_from_image(&self, image_path: &std::path::Path) -> Result<String, Box<dyn Error>> {
        let img = Image::from_path(image_path)?;
        let args = Args {
            lang: self.language.clone(),
            ..Default::default()
        };
        Ok(rusty_tesseract::image_to_string(&img, &args)?)
    }

    pub fn extract_text_from_image_data(&self, image_data: &[u8]) -> Result<String, Box<dyn Error>> {
        // Створити тимчасовий файл
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("ocr_temp_{}.png", std::process::id()));

        // Зберегти дані у файл
        let mut file = std::fs::File::create(&temp_file)?;
        file.write_all(image_data)?;
        drop(file); // Закрити файл

        // OCR з файлу
        let img = Image::from_path(&temp_file)?;
        let args = Args {
            lang: self.language.clone(),
            ..Default::default()
        };
        let result = rusty_tesseract::image_to_string(&img, &args)?;

        // Видалити тимчасовий файл
        let _ = std::fs::remove_file(&temp_file);

        Ok(result)
    }
}

use std::error::Error;
use std::io::Write;
use rusty_tesseract::{Args, Image};

pub struct OcrEngine {
    language: String,
    dpi: u32,
    psm: u8,
    oem: u8,
}

impl OcrEngine {
    pub fn new(language: &str) -> Result<Self, Box<dyn Error>> {
        Self::with_config(language, 300, 3, 3)
    }

    pub fn with_config(language: &str, dpi: u32, psm: u8, oem: u8) -> Result<Self, Box<dyn Error>> {
        Ok(OcrEngine {
            language: language.to_string(),
            dpi,
            psm,
            oem,
        })
    }

    fn build_args(&self) -> Args {
        Args {
            lang: self.language.clone(),
            dpi: Some(self.dpi as i32),
            psm: Some(self.psm as i32),
            oem: Some(self.oem as i32),
            ..Default::default()
        }
    }

    pub fn extract_text_from_image(&self, image_path: &std::path::Path) -> Result<String, Box<dyn Error>> {
        let img = Image::from_path(image_path)?;
        let args = self.build_args();
        Ok(rusty_tesseract::image_to_string(&img, &args)?)
    }

    #[allow(dead_code)]
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
        let args = self.build_args();
        let result = rusty_tesseract::image_to_string(&img, &args)?;

        // Видалити тимчасовий файл
        let _ = std::fs::remove_file(&temp_file);

        Ok(result)
    }
}

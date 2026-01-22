use std::error::Error;
use std::process::{Command, Stdio};

pub struct OcrEngine {
    language: String,
    dpi: u32,
    psm: u8,
    oem: u8,
    verbose: bool,
}

impl OcrEngine {
    pub fn with_config(language: &str, dpi: u32, psm: u8, oem: u8, verbose: bool) -> Result<Self, Box<dyn Error>> {
        Ok(OcrEngine {
            language: language.to_string(),
            dpi,
            psm,
            oem,
            verbose,
        })
    }

    pub fn extract_text_from_image(&self, image_path: &std::path::Path) -> Result<String, Box<dyn Error>> {
        let mut cmd = Command::new("tesseract");
        cmd.arg(image_path)
            .arg("stdout")
            .arg("-l").arg(&self.language)
            .arg("--dpi").arg(self.dpi.to_string())
            .arg("--psm").arg(self.psm.to_string())
            .arg("--oem").arg(self.oem.to_string());

        // hide stderr if not verbose
        if self.verbose {
            eprintln!("🔧 Tesseract: tesseract {} stdout -l {} --dpi {} --psm {} --oem {}",
                      image_path.display(),
                      self.language,
                      self.dpi,
                      self.psm,
                      self.oem
            );
        } else {
            cmd.stderr(Stdio::null());
        }

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Tesseract failed: {}", stderr).into());
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }
}

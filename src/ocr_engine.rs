use std::path::Path;
use std::error::Error;
use std::process::{Command, Stdio};

pub struct OcrEngine {
    language: String,
    dpi: u32,
    psm: u8,
    oem: u8,
    verbose: bool,
}

#[derive(Debug, Clone)]
pub struct OcrWordResult {
    pub text: String,
    pub confidence: f32,
    pub bbox: (u32, u32, u32, u32), // left, top, width, height
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

    pub fn extract_with_confidence(&self, image_path: &Path)
                                   -> Result<Vec<OcrWordResult>, Box<dyn Error>>
    {
        let output = Command::new("tesseract")
            .arg(image_path)
            .arg("stdout")
            .arg("-l").arg(&self.language)
            .arg("--dpi").arg(self.dpi.to_string())
            .arg("tsv") // ← key change
            .output()?;

        let tsv = String::from_utf8(output.stdout)?;
        let words = parse_tsv_output(&tsv)?;

        Ok(words)
    }

    pub fn get_average_confidence(&self, image_path: &Path) -> Result<f32, Box<dyn Error>> {
        let words = self.extract_with_confidence(image_path)?;

        if words.is_empty() {
            return Ok(0.0);
        }

        let avg = words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32;
        Ok(avg)
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

fn parse_tsv_output(tsv: &str) -> Result<Vec<OcrWordResult>, Box<dyn Error>> {
    let mut words = Vec::new();

    for line in tsv.lines().skip(1) { // skip header
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 12 && cols[0] == "5" { // level 5 = word
            let confidence = cols[10].parse::<f32>().unwrap_or(0.0);
            let text = cols[11].to_string();

            if !text.is_empty() {
                words.push(OcrWordResult {
                    text,
                    confidence,
                    bbox: (
                        cols[6].parse().unwrap_or(0),
                        cols[7].parse().unwrap_or(0),
                        cols[8].parse().unwrap_or(0),
                        cols[9].parse().unwrap_or(0),
                    ),
                });
            }
        }
    }

    Ok(words)
}
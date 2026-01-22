use std::error::Error;
use std::path::Path;
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
}

#[derive(Debug, Clone)]
pub struct OcrAnalysisResult {
    pub words: Vec<OcrWordResult>,
    pub avg_confidence: f32,
    pub detected_language: Option<String>,
    pub language_confidence: Option<f64>,
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
                                   -> Result<OcrAnalysisResult, Box<dyn Error>>
    {
        let output = Command::new("tesseract")
            .arg(image_path)
            .arg("stdout")
            .arg("-l").arg(&self.language)
            .arg("--dpi").arg(self.dpi.to_string())
            .arg("tsv")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Tesseract TSV failed: {}", stderr).into());
        }

        let tsv = String::from_utf8(output.stdout)?;
        let words = parse_tsv_output(&tsv)?;

        // Calculate average confidence
        let avg_confidence = if !words.is_empty() {
            words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
        } else {
            0.0
        };

        // Language detection from full text
        let full_text: String = words.iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let (detected_language, language_confidence) = if !full_text.is_empty() {
            match whatlang::detect(&full_text) {
                Some(info) => {
                    let lang_code = format!("{:?}", info.lang());
                    let confidence = info.confidence();

                    if self.verbose {
                        eprintln!("🌍 Detected: {} ({:.1}%)", lang_code, confidence * 100.0);
                    }

                    (Some(lang_code), Some(confidence))
                }
                None => (None, None),
            }
        } else {
            (None, None)
        };

        Ok(OcrAnalysisResult {
            words,
            avg_confidence,
            detected_language,
            language_confidence,
        })
    }

    pub fn extract_text_from_image(&self, image_path: &Path) -> Result<String, Box<dyn Error>> {
        let mut cmd = Command::new("tesseract");
        cmd.arg(image_path)
            .arg("stdout")
            .arg("-l").arg(&self.language)
            .arg("--dpi").arg(self.dpi.to_string())
            .arg("--psm").arg(self.psm.to_string())
            .arg("--oem").arg(self.oem.to_string());

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

    /// Check available Tesseract languages
    pub fn check_available_languages() -> Result<Vec<String>, Box<dyn Error>> {
        let output = Command::new("tesseract")
            .arg("--list-langs")
            .output()?;

        if !output.status.success() {
            return Err("Cannot check Tesseract languages".into());
        }

        let langs_text = String::from_utf8(output.stdout)?;
        let langs: Vec<String> = langs_text
            .lines()
            .skip(1)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(langs)
    }

    /// Validate requested languages against available ones
    pub fn validate_languages(requested: &str) -> Result<(), Box<dyn Error>> {
        let available = Self::check_available_languages()?;
        let requested_langs: Vec<&str> = requested.split('+').collect();

        let mut missing = Vec::new();
        for lang in requested_langs {
            if !available.contains(&lang.to_string()) {
                missing.push(lang);
            }
        }

        if !missing.is_empty() {
            eprintln!("⚠️  Missing language packs: {}", missing.join(", "));
            eprintln!("\n📦 Installation instructions:");
            eprintln!("  • Ubuntu/Debian: sudo apt install {}",
                      missing.iter().map(|l| format!("tesseract-ocr-{}", l)).collect::<Vec<_>>().join(" "));
            eprintln!("  • Windows: https://github.com/UB-Mannheim/tesseract/wiki");
            eprintln!("  • macOS: brew install tesseract-lang\n");

            return Err(format!("Missing language packs: {}", missing.join(", ")).into());
        }

        Ok(())
    }
}

fn parse_tsv_output(tsv: &str) -> Result<Vec<OcrWordResult>, Box<dyn Error>> {
    let mut words = Vec::new();

    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 12 && cols[0] == "5" {
            let confidence = cols[10].parse::<f32>().unwrap_or(0.0);
            let text = cols[11].to_string();

            if !text.is_empty() {
                words.push(OcrWordResult {
                    text,
                    confidence,
                });
            }
        }
    }

    Ok(words)
}

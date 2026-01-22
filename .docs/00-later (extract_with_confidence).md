<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

## 1. Виправлення помилки `Path`

Потрібно додати import на початку файлу:

```rust
// src/ocr_engine.rs
use std::error::Error;
use std::path::Path; // ← додати цей рядок
use std::process::{Command, Stdio};
```


## 2. Де і як використовувати `extract_with_confidence`

### Варіант А: Замість звичайного `extract_text_from_image` (коли потрібна якість)

```rust
// src/file_processors.rs
impl FileProcessor {
    fn process_image(
        &self,
        path: &Path,
        ocr_engine: &OcrEngine,
    ) -> Result<Vec<ProcessResult>, Box<dyn Error>> {
        // Отримати слова з confidence
        let words = ocr_engine.extract_with_confidence(path)?;
        
        // Фільтрувати тільки high-quality слова
        let high_quality_words: Vec<String> = words
            .iter()
            .filter(|w| w.confidence > 70.0) // поріг 70%
            .map(|w| w.text.clone())
            .collect();
        
        let text = high_quality_words.join(" ");
        
        // Розрахувати середню якість
        let avg_confidence = if !words.is_empty() {
            words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
        } else {
            0.0
        };
        
        println!("  📊 OCR quality: {:.1}% for {}", avg_confidence, path.display());
        
        Ok(vec![ProcessResult {
            file_type: FileType::from_path(path),
            page_count: 1,
            text,
        }])
    }
}
```


### Варіант Б: Паралельно - і текст, і статистика

```rust
// src/ocr_engine.rs
pub struct DetailedOcrResult {
    pub text: String,
    pub words: Vec<OcrWordResult>,
    pub avg_confidence: f32,
    pub low_confidence_count: usize,
}

impl OcrEngine {
    pub fn extract_with_details(&self, image_path: &Path) 
        -> Result<DetailedOcrResult, Box<dyn Error>> 
    {
        let words = self.extract_with_confidence(image_path)?;
        
        let text = words.iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        
        let avg_confidence = if !words.is_empty() {
            words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
        } else {
            0.0
        };
        
        let low_confidence_count = words.iter()
            .filter(|w| w.confidence < 70.0)
            .count();
        
        Ok(DetailedOcrResult {
            text,
            words,
            avg_confidence,
            low_confidence_count,
        })
    }
}
```

Використання:

```rust
// src/main.rs або file_processors.rs
let details = ocr_engine.extract_with_details(&path)?;

println!("Text: {}", details.text);
println!("Quality: {:.1}%", details.avg_confidence);
println!("Low confidence words: {}", details.low_confidence_count);

if details.avg_confidence < 60.0 {
    eprintln!("⚠️  Warning: Low OCR quality for {}", path.display());
}
```


### Варіант В: Тільки для звітів (рекомендую для початку)

Додати поле до `OcrResult`:

```rust
// src/main.rs
#[derive(Debug, Clone, serde::Serialize)]
struct OcrResult {
    filename: String,
    file_type: String,
    page_count: usize,
    text: String,
    processing_time_ms: u128,
    error: Option<String>,
    metadata: HashMap<String, String>,
    ocr_confidence: Option<f32>, // ← нове поле
}
```

Використання в `process_single_file`:

```rust
fn process_single_file(
    path: PathBuf,
    ocr_engine: &OcrEngine,
    file_processor: &FileProcessor,
    pb: &Arc<Mutex<ProgressBar>>,
) -> Vec<OcrResult> {
    // ...
    
    match file_processor.process_file(&path, ocr_engine) {
        Ok(process_results) => {
            for result in process_results {
                // Додатково розрахувати confidence для зображень
                let ocr_confidence = if matches!(FileType::from_path(&path), FileType::Image(_)) {
                    ocr_engine.extract_with_confidence(&path)
                        .ok()
                        .map(|words| {
                            if !words.is_empty() {
                                words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32
                            } else {
                                0.0
                            }
                        })
                } else {
                    None
                };
                
                results.push(OcrResult {
                    // ... інші поля
                    ocr_confidence,
                    // ...
                });
            }
        }
        // ...
    }
}
```


### Варіант Г: CLI опція (найпрактичніше)

```rust
// src/main.rs
#[derive(Parser, Debug)]
struct Cli {
    // ... інші поля ...
    
    /// Enable detailed OCR quality analysis
    #[arg(long)]
    analyze_quality: bool,
}
```

Використання:

```rust
fn main() -> Result<(), Box<dyn Error>> {
    // ...
    
    if cli.analyze_quality {
        println!("\n📊 OCR Quality Analysis");
        println!("{'─':<50}");
        
        for file in &files {
            if matches!(FileType::from_path(file), FileType::Image(_)) {
                let words = ocr_engine.extract_with_confidence(file)?;
                let avg = words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32;
                
                println!("{:<40} {:.1}%", 
                    file.file_name().unwrap().to_string_lossy(),
                    avg
                );
            }
        }
    }
    
    // Звичайна обробка...
}
```


## Повний приклад додавання (мінімальний)

```rust
// src/ocr_engine.rs - додати структуру та метод
#[derive(Debug, Clone)]
pub struct OcrWordResult {
    pub text: String,
    pub confidence: f32,
}

impl OcrEngine {
    pub fn get_average_confidence(&self, image_path: &Path) -> Result<f32, Box<dyn Error>> {
        let words = self.extract_with_confidence(image_path)?;
        
        if words.is_empty() {
            return Ok(0.0);
        }
        
        let avg = words.iter().map(|w| w.confidence).sum::<f32>() / words.len() as f32;
        Ok(avg)
    }
}
```

Використання (найпростіше):

```bash
# Звичайний режим
cargo run -- -i ./input -o ./output

# З аналізом якості (додати прапорець)
cargo run -- -i ./input -o ./output --analyze-quality
```

**Рекомендую почати з Варіанта Г** - опціональна CLI функція, не ламає поточний функціонал, але дає корисну інформацію коли потрібно.
<span style="display:none">[^1]</span>

<div align="center">⁂</div>

[^1]: 02-multilang.md


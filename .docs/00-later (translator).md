<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# реалізуємо зголом

Окей, реалізуємо переклад! Ось повна інтеграція:

## 1. Додати залежності в `Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies

# Translation
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
async-openai = "0.27"
```


## 2. Створити модуль `src/translator.rs`:

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Copy)]
pub enum TranslationService {
    LibreTranslate,
    OpenAI,
}

#[derive(Serialize)]
struct LibreTranslateRequest {
    q: String,
    source: String,
    target: String,
}

#[derive(Deserialize)]
struct LibreTranslateResponse {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

pub async fn translate_libretranslate(
    text: &str,
    target_lang: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = Client::new();
    let url = "https://libretranslate.com/translate";
    
    let request = LibreTranslateRequest {
        q: text.to_string(),
        source: "auto".to_string(),
        target: target_lang.to_string(),
    };
    
    let response = client
        .post(url)
        .json(&request)
        .send()
        .await?
        .json::<LibreTranslateResponse>()
        .await?;
    
    Ok(response.translated_text)
}

pub async fn translate_openai(
    text: &str,
    target_lang: &str,
    api_key: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    use async_openai::{
        types::{
            ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
            ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
        },
        Client,
    };

    let client = Client::new().with_api_key(api_key);

    let system_msg = ChatCompletionRequestSystemMessageArgs::default()
        .content("You are a professional translator. Translate the given text accurately while preserving formatting and meaning.")
        .build()?;

    let user_msg = ChatCompletionRequestUserMessageArgs::default()
        .content(format!("Translate this text to {}: {}", target_lang, text))
        .build()?;

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4o-mini")
        .messages(vec![
            ChatCompletionRequestMessage::System(system_msg),
            ChatCompletionRequestMessage::User(user_msg),
        ])
        .build()?;

    let response = client.chat().create(request).await?;
    
    let translated = response.choices[^0]
        .message
        .content
        .clone()
        .ok_or("No translation received")?;

    Ok(translated)
}

pub async fn translate_text(
    text: &str,
    target_lang: &str,
    service: TranslationService,
    api_key: Option<&str>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    match service {
        TranslationService::LibreTranslate => {
            translate_libretranslate(text, target_lang).await
        }
        TranslationService::OpenAI => {
            let key = api_key.ok_or("OpenAI API key required for OpenAI translation")?;
            translate_openai(text, target_lang, key).await
        }
    }
}
```


## 3. Оновити `src/main.rs`:

```rust
mod file_processors;
mod ocr_engine;
mod utils;
mod pdf_creator;
mod translator;  // ✅ Додати

use translator::{translate_text, TranslationService};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum TranslationServiceType {
    /// LibreTranslate (free, good quality)
    Libretranslate,
    /// OpenAI GPT-4 (best quality, requires API key)
    Openai,
}

#[derive(Parser, Debug)]
struct Cli {
    // ... existing fields

    /// Translate recognized text to target language (e.g., en, uk, de)
    #[arg(long)]
    translate_to: Option<String>,

    /// Translation service to use
    #[arg(long, value_enum, default_value = "libretranslate")]
    translation_service: TranslationServiceType,

    /// OpenAI API key (required for openai service, or set OPENAI_API_KEY env var)
    #[arg(long, env = "OPENAI_API_KEY")]
    openai_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // ... existing code до results

    // ✅ Translation
    if let Some(target_lang) = &cli.translate_to {
        println!("\n🌍 Translating to '{}'...", target_lang);
        
        let service = match cli.translation_service {
            TranslationServiceType::Libretranslate => TranslationService::LibreTranslate,
            TranslationServiceType::Openai => TranslationService::OpenAI,
        };

        // Validate API key for OpenAI
        if matches!(service, TranslationService::OpenAI) && cli.openai_key.is_none() {
            eprintln!("\n❌ OpenAI API key required!");
            eprintln!("Set OPENAI_API_KEY environment variable or use --openai-key");
            return Err("Missing OpenAI API key".into());
        }

        let mut translated_count = 0;
        let mut failed_count = 0;

        for result in &mut results {
            if result.error.is_none() && !result.text.is_empty() {
                let filename = result.filename.clone();
                print!("  ⏳ {}...", filename);
                std::io::Write::flush(&mut std::io::stdout())?;

                match translate_text(
                    &result.text,
                    target_lang,
                    service,
                    cli.openai_key.as_deref(),
                )
                .await
                {
                    Ok(translated) => {
                        result.text = translated;
                        translated_count += 1;
                        println!("\r  ✓ {}", filename);
                    }
                    Err(e) => {
                        failed_count += 1;
                        eprintln!("\r  ✗ {}: {}", filename, e);
                    }
                }
            }
        }

        println!(
            "\n📊 Translation complete: {} successful, {} failed",
            translated_count, failed_count
        );
    }

    // ... existing code (save results, searchable PDF, etc.)

    Ok(())
}
```


## 4. Змінити `fn main()` на async:

Видали `fn main()` і зроби:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // ... весь існуючий код залишається
}
```


## 5. Використання:

```bash
# LibreTranslate (безкоштовно):
cargo run -- --input .\images\input --output .\images\output --translate-to en

# OpenAI (найкраща якість):
$env:OPENAI_API_KEY="sk-proj-..."
cargo run -- --input .\images\input --output .\images\output --translate-to en --translation-service openai

# З українського на англійську + searchable PDF:
cargo run -- --input .\images\input --output .\images\output --translate-to en --searchable-pdf

# Переклад на німецьку:
cargo run -- --input .\images\input --output .\images\output --translate-to de

# Допомога:
cargo run -- --help
```

Готово! Тепер є повноцінний переклад з вибором сервісу! 🌍🚀
<span style="display:none">[^1][^2][^3][^4]</span>

<div align="center">⁂</div>

[^1]: 01-initial.md

[^2]: image.jpg

[^3]: image.jpg

[^4]: image.jpg


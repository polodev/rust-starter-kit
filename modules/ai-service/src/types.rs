//! What callers pass in and get back.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Provider {
    OpenAi,
    Claude,
    Gemini,
}

impl Provider {
    pub const ALL: [Provider; 3] = [Provider::Claude, Provider::OpenAi, Provider::Gemini];

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_lowercase().as_str() {
            "openai" | "open-ai" | "gpt" => Some(Self::OpenAi),
            "claude" | "anthropic" => Some(Self::Claude),
            "gemini" | "google" => Some(Self::Gemini),
            _ => None,
        }
    }

    /// The short name stored in the log and used in `.env` (`AI_DEFAULT_PROVIDER`).
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Claude => "Claude",
            Self::Gemini => "Gemini",
        }
    }

    pub fn key_variable(self) -> &'static str {
        match self {
            Self::OpenAi => "OPENAI_API_KEY",
            Self::Claude => "ANTHROPIC_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Claude => "https://api.anthropic.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta",
        }
    }

    /// Used when `.env` names no model. Model names change often: set `*_MODEL` in `.env`.
    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-5.6-luna",
            Self::Claude => "claude-sonnet-5-5",
            Self::Gemini => "gemini-3.8-flash",
        }
    }
}

/// A picture sent to the model together with the prompt (for reading receipts, photos, screenshots).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInput {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// A text request. Build it step by step:
///
/// ```ignore
/// Request::new("Translate to Bangla: Good morning").system("Reply with the translation only.")
/// Request::new("Read this receipt").image("image/jpeg", bytes).provider(Provider::Gemini)
/// ```
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub prompt: String,
    pub system: Option<String>,
    pub images: Vec<ImageInput>,
    /// Which provider; the configured default when `None`.
    pub provider: Option<Provider>,
    /// Which model; the configured model of the provider when `None`.
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    /// Ask for a JSON reply (the adapters use each provider's own way to do it).
    pub json: bool,
    /// What the call is for, shown in the log (`passport-scan`, `translate-page`).
    pub purpose: Option<String>,
    /// Keeps the prompt and the reply out of the log (passports, IDs, anything personal). Images are never logged.
    pub sensitive: bool,
}

impl Request {
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            ..Self::default()
        }
    }

    pub fn system(mut self, system: &str) -> Self {
        self.system = Some(system.to_string());
        self
    }

    pub fn image(mut self, mime: &str, bytes: Vec<u8>) -> Self {
        self.images.push(ImageInput {
            mime: mime.to_string(),
            bytes,
        });
        self
    }

    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn json(mut self) -> Self {
        self.json = true;
        self
    }

    pub fn purpose(mut self, purpose: &str) -> Self {
        self.purpose = Some(purpose.to_string());
        self
    }

    pub fn sensitive(mut self) -> Self {
        self.sensitive = true;
        self
    }
}

/// A model's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub text: String,
    pub provider: Provider,
    pub model: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Estimated dollars, when prices are set for the provider in `.env`.
    pub cost: Option<f64>,
}

impl Reply {
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

/// An image to generate (OpenAI).
#[derive(Debug, Clone, Default)]
pub struct ImageRequest {
    pub prompt: String,
    pub model: Option<String>,
    /// For example `1024x1024`, `1024x1536`, `1536x1024`.
    pub size: Option<String>,
    pub quality: Option<String>,
    pub count: Option<u8>,
    pub purpose: Option<String>,
}

impl ImageRequest {
    pub fn new(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            ..Self::default()
        }
    }

    pub fn size(mut self, size: &str) -> Self {
        self.size = Some(size.to_string());
        self
    }

    pub fn quality(mut self, quality: &str) -> Self {
        self.quality = Some(quality.to_string());
        self
    }

    pub fn model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn count(mut self, count: u8) -> Self {
        self.count = Some(count);
        self
    }

    pub fn purpose(mut self, purpose: &str) -> Self {
        self.purpose = Some(purpose.to_string());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    pub mime: String,
    /// How the provider reworded the prompt, when it says so.
    pub revised_prompt: Option<String>,
}

impl GeneratedImage {
    pub fn extension(&self) -> &'static str {
        match self.mime.as_str() {
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => "png",
        }
    }
}

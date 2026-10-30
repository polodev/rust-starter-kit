//! Settings from `settings.ai` in `config/<environment>.yaml`, whose values come from `.env`:
//!
//! ```text
//! AI_DEFAULT_PROVIDER=claude          # claude | openai | gemini (empty: the first provider that has a key)
//! AI_DEFAULT_MODEL=                   # optional: overrides the model of the default provider
//! ANTHROPIC_API_KEY=...   CLAUDE_MODEL=claude-sonnet-5-5
//! OPENAI_API_KEY=...      OPENAI_MODEL=...      OPENAI_IMAGE_MODEL=...
//! GEMINI_API_KEY=...      GEMINI_MODEL=...
//! ```
//!
//! A key is all a provider needs; every model has a default that `.env` can change.

use loco_rs::{app::AppContext, Error, Result};
use serde::{Deserialize, Deserializer};

use crate::types::Provider;

fn null_as_empty<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    Ok(Option::<serde_json::Value>::deserialize(deserializer)?
        .map(|value| match value {
            serde_json::Value::String(text) => text,
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        })
        .unwrap_or_default())
}

/// A number that may arrive as a number, as text (from an environment variable) or empty.
fn flexible_number<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<f64>, D::Error> {
    Ok(
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            Some(serde_json::Value::Number(number)) => number.as_f64(),
            Some(serde_json::Value::String(text)) => text.trim().parse().ok(),
            _ => None,
        },
    )
}

fn flexible_bool<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<bool, D::Error> {
    Ok(
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            Some(serde_json::Value::Bool(flag)) => flag,
            Some(serde_json::Value::String(text)) => !matches!(
                text.trim().to_lowercase().as_str(),
                "false" | "0" | "no" | "off"
            ),
            _ => true,
        },
    )
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct ProviderConfig {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub api_key: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub model: String,
    /// Only needed for a proxy or a compatible service; the provider's own address is the default.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub base_url: String,
    /// US dollars per million input tokens, to estimate cost. Empty: no estimate.
    #[serde(default, deserialize_with = "flexible_number")]
    pub input_per_million: Option<f64>,
    #[serde(default, deserialize_with = "flexible_number")]
    pub output_per_million: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ImageConfig {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub model: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub size: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub quality: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub output_format: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AiConfig {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub default_provider: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub default_model: String,
    #[serde(default, deserialize_with = "flexible_number")]
    pub max_tokens: Option<f64>,
    #[serde(default, deserialize_with = "flexible_number")]
    pub timeout_seconds: Option<f64>,
    /// `false` keeps prompts and replies out of the log (only sizes, status and cost are stored).
    #[serde(default = "default_true", deserialize_with = "flexible_bool")]
    pub log_content: bool,
    #[serde(default)]
    pub openai: ProviderConfig,
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub gemini: ProviderConfig,
    #[serde(default)]
    pub image: ImageConfig,
}

fn default_true() -> bool {
    true
}

impl AiConfig {
    pub fn load(ctx: &AppContext) -> Result<Self> {
        let value = ctx
            .config
            .settings
            .as_ref()
            .and_then(|settings| settings.get("ai"))
            .ok_or_else(|| Error::string("AI is not configured: add settings.ai to config/<environment>.yaml (see modules/ai-service/README.md)"))?;
        Self::from_value(value.clone())
    }

    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        let config: Self = serde_json::from_value(value)
            .map_err(|error| Error::string(&format!("settings.ai is not valid: {error}")))?;
        if !config.default_provider.trim().is_empty()
            && Provider::parse(&config.default_provider).is_none()
        {
            return Err(Error::string(&format!(
                "AI_DEFAULT_PROVIDER `{}` is not claude, openai or gemini",
                config.default_provider
            )));
        }
        Ok(config)
    }

    fn provider_config(&self, provider: Provider) -> &ProviderConfig {
        match provider {
            Provider::OpenAi => &self.openai,
            Provider::Claude => &self.claude,
            Provider::Gemini => &self.gemini,
        }
    }

    pub fn has_key(&self, provider: Provider) -> bool {
        !self.provider_config(provider).api_key.trim().is_empty()
    }

    pub fn api_key(&self, provider: Provider) -> Result<&str> {
        if self.has_key(provider) {
            Ok(self.provider_config(provider).api_key.trim())
        } else {
            Err(Error::string(&format!(
                "{} has no API key: set {} in .env",
                provider.label(),
                provider.key_variable()
            )))
        }
    }

    pub fn base_url(&self, provider: Provider) -> String {
        let configured = self
            .provider_config(provider)
            .base_url
            .trim()
            .trim_end_matches('/');
        if configured.is_empty() {
            provider.default_base_url().to_string()
        } else {
            configured.to_string()
        }
    }

    /// The provider used when a request does not name one: `AI_DEFAULT_PROVIDER`, else the first with a key.
    pub fn default_provider(&self) -> Result<Provider> {
        if let Some(named) = Provider::parse(&self.default_provider) {
            self.api_key(named)?;
            return Ok(named);
        }
        [Provider::Claude, Provider::OpenAi, Provider::Gemini]
            .into_iter()
            .find(|provider| self.has_key(*provider))
            .ok_or_else(|| {
                Error::string("no AI provider is configured: set ANTHROPIC_API_KEY, OPENAI_API_KEY or GEMINI_API_KEY in .env")
            })
    }

    /// The model to use: the request's own, else `AI_DEFAULT_MODEL` (for the default provider only), else the
    /// provider's `*_MODEL`, else the built-in default.
    pub fn model(&self, provider: Provider, requested: Option<&str>) -> String {
        if let Some(model) = requested.map(str::trim).filter(|model| !model.is_empty()) {
            return model.to_string();
        }
        let is_default = self.default_provider().ok() == Some(provider);
        if is_default && !self.default_model.trim().is_empty() {
            return self.default_model.trim().to_string();
        }
        let configured = self.provider_config(provider).model.trim();
        if configured.is_empty() {
            provider.default_model().to_string()
        } else {
            configured.to_string()
        }
    }

    pub fn max_tokens(&self, requested: Option<u32>) -> u32 {
        requested
            .unwrap_or_else(|| self.max_tokens.map_or(2048, |tokens| tokens as u32))
            .max(1)
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(
            self.timeout_seconds
                .filter(|secs| *secs > 0.0)
                .unwrap_or(60.0),
        )
    }

    /// Estimated dollars for a call, when the prices of that provider are set.
    pub fn cost(&self, provider: Provider, input_tokens: u32, output_tokens: u32) -> Option<f64> {
        let config = self.provider_config(provider);
        let (input, output) = (config.input_per_million?, config.output_per_million?);
        Some(
            f64::from(input_tokens) / 1_000_000.0 * input
                + f64::from(output_tokens) / 1_000_000.0 * output,
        )
    }

    pub fn image_model(&self) -> String {
        let model = self.image.model.trim();
        if model.is_empty() {
            "gpt-image-2".to_string()
        } else {
            model.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn config(extra: serde_json::Value) -> AiConfig {
        let mut base = json!({
            "claude": { "api_key": "ck" }, "openai": { "api_key": "ok", "model": "gpt-x" }, "gemini": { "api_key": "" }
        });
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        AiConfig::from_value(base).unwrap()
    }

    #[test]
    fn the_default_provider_comes_from_env_or_the_first_key() {
        assert_eq!(
            config(json!({})).default_provider().unwrap(),
            Provider::Claude,
            "first with a key"
        );
        assert_eq!(
            config(json!({ "default_provider": "openai" }))
                .default_provider()
                .unwrap(),
            Provider::OpenAi
        );
        assert_eq!(
            config(json!({ "default_provider": "Anthropic" }))
                .default_provider()
                .unwrap(),
            Provider::Claude
        );
        let missing = config(json!({ "default_provider": "gemini" }))
            .default_provider()
            .unwrap_err()
            .to_string();
        assert!(missing.contains("GEMINI_API_KEY"), "{missing}");
        assert!(AiConfig::from_value(json!({ "default_provider": "llama" })).is_err());
        let none = AiConfig::from_value(json!({}))
            .unwrap()
            .default_provider()
            .unwrap_err()
            .to_string();
        assert!(none.contains("no AI provider"));
    }

    #[test]
    fn models_follow_request_then_default_model_then_provider_then_builtin() {
        let cfg = config(json!({ "default_provider": "openai", "default_model": "gpt-default" }));
        assert_eq!(
            cfg.model(Provider::OpenAi, Some("gpt-explicit")),
            "gpt-explicit"
        );
        assert_eq!(
            cfg.model(Provider::OpenAi, None),
            "gpt-default",
            "AI_DEFAULT_MODEL applies to the default provider"
        );
        assert_eq!(
            cfg.model(Provider::Claude, None),
            Provider::Claude.default_model(),
            "but not to the others"
        );
        let plain = config(json!({ "default_provider": "openai" }));
        assert_eq!(
            plain.model(Provider::OpenAi, None),
            "gpt-x",
            "the provider's own model"
        );
        assert_eq!(plain.model(Provider::OpenAi, Some("  ")), "gpt-x");
    }

    #[test]
    fn numbers_cost_and_flags_accept_env_style_text() {
        let cfg = AiConfig::from_value(json!({
            "max_tokens": "512", "timeout_seconds": "", "log_content": "false",
            "claude": { "api_key": "k", "input_per_million": "3", "output_per_million": 15 }
        }))
        .unwrap();
        assert_eq!(cfg.max_tokens(None), 512);
        assert_eq!(cfg.max_tokens(Some(100)), 100);
        assert_eq!(cfg.timeout(), std::time::Duration::from_secs(60));
        assert!(!cfg.log_content);
        let cost = cfg.cost(Provider::Claude, 1_000_000, 100_000).unwrap();
        assert!((cost - 4.5).abs() < 1e-9, "{cost}");
        assert_eq!(
            cfg.cost(Provider::OpenAi, 10, 10),
            None,
            "no prices, no estimate"
        );
        assert!(
            AiConfig::from_value(json!({})).unwrap().log_content,
            "logging content is on by default"
        );
    }
}

//! The wrapper other modules call. Every call is written to `ai_api_logs`, whether it worked or not.
//!
//! ```ignore
//! let ai = module_ai_service::AiService::new(&ctx)?;
//! let reply = ai.chat(Request::new("Say hi in Bangla")).await?;
//! let (data, _) = ai.json(Request::new("Read this passport").image("image/jpeg", bytes).sensitive()).await?;
//! let pictures = ai.generate_image(ImageRequest::new("a red bicycle").size("1024x1024")).await?;
//! ```

use std::time::Instant;

use chrono::Utc;
use loco_rs::{app::AppContext, Error, Result};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::Value;

use crate::{
    config::AiConfig,
    entities::ai_api_logs,
    providers::{self, Call, ImageSource},
    types::{GeneratedImage, ImageRequest, Provider, Reply, Request},
};

/// What a log row needs, collected while a call runs.
struct Entry {
    kind: &'static str,
    provider: Provider,
    model: String,
    purpose: Option<String>,
    sensitive: bool,
    image_count: usize,
    prompt: String,
}

struct Outcome {
    http_status: Option<u16>,
    duration_ms: u128,
    input_tokens: u32,
    output_tokens: u32,
    cost: Option<f64>,
    response: Option<String>,
    error: Option<String>,
}

#[derive(Clone)]
pub struct AiService {
    db: DatabaseConnection,
    config: AiConfig,
    client: reqwest::Client,
}

impl AiService {
    pub fn new(ctx: &AppContext) -> Result<Self> {
        Self::with_config(ctx.db.clone(), AiConfig::load(ctx)?)
    }

    pub fn with_config(db: DatabaseConnection, config: AiConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout())
            .build()
            .map_err(|error| Error::string(&error.to_string()))?;
        Ok(Self { db, config, client })
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }

    /// Sends a text (or text and image) request and returns the answer.
    pub async fn chat(&self, request: Request) -> Result<Reply> {
        let provider = match request.provider {
            Some(provider) => provider,
            None => self.config.default_provider()?,
        };
        let key = self.config.api_key(provider)?.to_string();
        let model = self.config.model(provider, request.model.as_deref());
        let call = providers::text_call(provider, &self.config, &request, &model, &key);
        let entry = Entry {
            kind: if request.images.is_empty() {
                "text"
            } else {
                "vision"
            },
            provider,
            model: model.clone(),
            purpose: request.purpose.clone(),
            sensitive: request.sensitive,
            image_count: request.images.len(),
            prompt: request.prompt.clone(),
        };

        let started = Instant::now();
        let result = self.send(&call).await;
        let elapsed = started.elapsed().as_millis();
        let (http_status, body) = match result {
            Ok(done) => done,
            Err((http_status, message)) => {
                self.record(
                    &entry,
                    Outcome {
                        http_status,
                        duration_ms: elapsed,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost: None,
                        response: None,
                        error: Some(message.clone()),
                    },
                )
                .await;
                return Err(Error::string(&format!("{}: {message}", provider.label())));
            }
        };
        match providers::read_text(provider, &body) {
            Ok(answer) => {
                let cost = self
                    .config
                    .cost(provider, answer.input_tokens, answer.output_tokens);
                self.record(
                    &entry,
                    Outcome {
                        http_status: Some(http_status),
                        duration_ms: elapsed,
                        input_tokens: answer.input_tokens,
                        output_tokens: answer.output_tokens,
                        cost,
                        response: Some(answer.text.clone()),
                        error: None,
                    },
                )
                .await;
                Ok(Reply {
                    text: answer.text,
                    provider,
                    model,
                    input_tokens: answer.input_tokens,
                    output_tokens: answer.output_tokens,
                    cost,
                })
            }
            Err(message) => {
                self.record(
                    &entry,
                    Outcome {
                        http_status: Some(http_status),
                        duration_ms: elapsed,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost: None,
                        response: None,
                        error: Some(message.clone()),
                    },
                )
                .await;
                Err(Error::string(&message))
            }
        }
    }

    /// Like [`chat`](Self::chat) with a JSON answer: asks for JSON and parses it, also when the model wrapped it
    /// in a markdown fence or added a sentence around it. A reply that is not JSON is an error (and is logged).
    pub async fn json(&self, mut request: Request) -> Result<(Value, Reply)> {
        request.json = true;
        let reply = self.chat(request).await?;
        match extract_json(&reply.text) {
            Some(value) => Ok((value, reply)),
            None => Err(Error::string(&format!(
                "{} did not return valid JSON (see the AI API logs)",
                reply.provider.label()
            ))),
        }
    }

    /// Generates one or more images with OpenAI.
    pub async fn generate_image(&self, request: ImageRequest) -> Result<Vec<GeneratedImage>> {
        let provider = Provider::OpenAi;
        let key = self.config.api_key(provider)?.to_string();
        let model = request
            .model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(|| self.config.image_model());
        let call = providers::image_call(&self.config, &request, &model, &key);
        let entry = Entry {
            kind: "image",
            provider,
            model,
            purpose: request.purpose.clone(),
            sensitive: false,
            image_count: 0,
            prompt: request.prompt.clone(),
        };

        let started = Instant::now();
        let outcome = match self.send(&call).await {
            Err((http_status, message)) => Err((http_status, message)),
            Ok((http_status, body)) => {
                match providers::read_images(&body, &self.config.image.output_format) {
                    Err(message) => Err((Some(http_status), message)),
                    Ok(sources) => self.collect_images(http_status, sources).await,
                }
            }
        };
        let elapsed = started.elapsed().as_millis();
        match outcome {
            Ok((http_status, images)) => {
                let summary = format!("{} image(s) generated", images.len());
                self.record(
                    &entry,
                    Outcome {
                        http_status: Some(http_status),
                        duration_ms: elapsed,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost: None,
                        response: Some(summary),
                        error: None,
                    },
                )
                .await;
                Ok(images)
            }
            Err((http_status, message)) => {
                self.record(
                    &entry,
                    Outcome {
                        http_status,
                        duration_ms: elapsed,
                        input_tokens: 0,
                        output_tokens: 0,
                        cost: None,
                        response: None,
                        error: Some(message.clone()),
                    },
                )
                .await;
                Err(Error::string(&format!("OpenAI: {message}")))
            }
        }
    }

    async fn collect_images(
        &self,
        http_status: u16,
        sources: Vec<ImageSource>,
    ) -> std::result::Result<(u16, Vec<GeneratedImage>), (Option<u16>, String)> {
        let mut images = Vec::new();
        for source in sources {
            match source {
                ImageSource::Inline(image) => images.push(image),
                ImageSource::Url {
                    url,
                    revised_prompt,
                } => {
                    let response = self
                        .client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|e| (Some(http_status), e.to_string()))?;
                    let mime = response
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("image/png")
                        .to_string();
                    let bytes = response
                        .bytes()
                        .await
                        .map_err(|e| (Some(http_status), e.to_string()))?;
                    images.push(GeneratedImage {
                        bytes: bytes.to_vec(),
                        mime,
                        revised_prompt,
                    });
                }
            }
        }
        Ok((http_status, images))
    }

    /// Sends the call. `Ok` is a 2xx answer with a JSON body; `Err` carries the status (when there was one) and a message.
    async fn send(&self, call: &Call) -> std::result::Result<(u16, Value), (Option<u16>, String)> {
        let mut builder = self.client.post(&call.url).json(&call.body);
        for (name, value) in &call.headers {
            builder = builder.header(*name, value);
        }
        let response = builder.send().await.map_err(|error| {
            let reason = if error.is_timeout() {
                "the request timed out".to_string()
            } else {
                error.without_url().to_string()
            };
            (None, reason)
        })?;
        let status = response.status().as_u16();
        let raw = response
            .text()
            .await
            .map_err(|error| (Some(status), error.to_string()))?;
        let body: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            if body.is_null() {
                return Err((
                    Some(status),
                    format!(
                        "the answer is not JSON: {}",
                        raw.chars().take(200).collect::<String>()
                    ),
                ));
            }
            Ok((status, body))
        } else {
            Err((Some(status), providers::error_message(&body, &raw)))
        }
    }

    /// Writes the log row. A problem with the log must never break the call, so it is only reported.
    async fn record(&self, entry: &Entry, outcome: Outcome) {
        let keep_content = self.config.log_content && !entry.sensitive;
        let now = Utc::now().fixed_offset();
        let row = ai_api_logs::ActiveModel {
            kind: Set(entry.kind.to_string()),
            provider: Set(entry.provider.name().to_string()),
            model: Set(entry.model.clone()),
            purpose: Set(entry.purpose.clone()),
            status: Set(if outcome.error.is_some() {
                ai_api_logs::FAILED
            } else {
                ai_api_logs::SUCCESS
            }
            .to_string()),
            http_status: Set(outcome.http_status),
            duration_ms: Set(outcome.duration_ms.min(u128::from(u32::MAX)) as u32),
            input_tokens: Set(outcome.input_tokens),
            output_tokens: Set(outcome.output_tokens),
            cost: Set(outcome.cost),
            image_count: Set(entry.image_count.min(usize::from(u16::MAX)) as u16),
            sensitive: Set(entry.sensitive),
            prompt: Set(keep_content.then(|| entry.prompt.clone())),
            response: Set(outcome
                .response
                .filter(|_| keep_content || entry.kind == "image")),
            // An error text is the provider's own and does not echo the prompt, so it is always kept.
            error: Set(outcome.error),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        };
        if let Err(error) = row.insert(&self.db).await {
            tracing::warn!(%error, "could not write the AI API log");
        }
    }
}

/// Finds the JSON value in a model's text: the whole text, a fenced block, or the outermost `{...}` / `[...]`.
pub fn extract_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Some(value);
    }
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```JSON"))
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|rest| rest.trim_end().trim_end_matches("```").trim());
    if let Some(value) = unfenced.and_then(|inner| serde_json::from_str(inner).ok()) {
        return Some(value);
    }
    [('{', '}'), ('[', ']')]
        .into_iter()
        .find_map(|(open, close)| {
            let (start, end) = (trimmed.find(open)?, trimmed.rfind(close)?);
            (start < end)
                .then(|| serde_json::from_str(&trimmed[start..=end]).ok())
                .flatten()
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn json_is_found_in_plain_fenced_and_chatty_replies() {
        let expected = json!({ "surname": "KHAN", "age": 30 });
        for text in [
            r#"{"surname":"KHAN","age":30}"#,
            "```json\n{\"surname\":\"KHAN\",\"age\":30}\n```",
            "```\n{\"surname\":\"KHAN\",\"age\":30}\n```",
            "Here is the result:\n{\"surname\":\"KHAN\",\"age\":30}\nHope this helps!",
        ] {
            assert_eq!(extract_json(text), Some(expected.clone()), "{text}");
        }
        assert_eq!(extract_json("[1, 2]"), Some(json!([1, 2])));
        assert_eq!(extract_json("sorry, I cannot read it"), None);
        assert_eq!(extract_json("{broken"), None);
    }
}

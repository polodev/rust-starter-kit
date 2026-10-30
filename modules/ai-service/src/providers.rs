//! The three text providers and OpenAI images, as plain functions: build the HTTP request, read the answer.
//! Nothing here touches the network, so every shape is covered by unit tests.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

use crate::{
    config::AiConfig,
    types::{GeneratedImage, ImageRequest, Provider, Request},
};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const JSON_ONLY: &str =
    "Reply with one valid JSON value and nothing else: no explanation, no markdown fences.";

/// An HTTP request ready to send.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Value,
}

/// What a provider answered, reduced to what callers need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

fn tokens(value: Option<&Value>) -> u32 {
    value
        .and_then(Value::as_u64)
        .map_or(0, |count| count.min(u64::from(u32::MAX)) as u32)
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

pub fn text_call(
    provider: Provider,
    config: &AiConfig,
    request: &Request,
    model: &str,
    key: &str,
) -> Call {
    let base = config.base_url(provider);
    let max_tokens = config.max_tokens(request.max_tokens);
    match provider {
        Provider::OpenAi => {
            let mut messages = Vec::new();
            if let Some(system) = &request.system {
                messages.push(json!({ "role": "system", "content": system }));
            }
            let mut content = vec![json!({ "type": "text", "text": request.prompt })];
            for image in &request.images {
                content.push(json!({ "type": "image_url", "image_url": { "url": data_url(&image.mime, &image.bytes) } }));
            }
            messages.push(json!({ "role": "user", "content": content }));
            let mut body = json!({ "model": model, "messages": messages, "max_completion_tokens": max_tokens });
            if let Some(temperature) = request.temperature {
                body["temperature"] = json!(temperature);
            }
            if request.json {
                body["response_format"] = json!({ "type": "json_object" });
            }
            Call {
                url: format!("{base}/chat/completions"),
                headers: vec![("authorization", format!("Bearer {key}"))],
                body,
            }
        }
        Provider::Claude => {
            // Claude has no JSON mode: the instruction goes into the system prompt.
            let system = match (&request.system, request.json) {
                (Some(system), true) => Some(format!("{system}\n\n{JSON_ONLY}")),
                (Some(system), false) => Some(system.clone()),
                (None, true) => Some(JSON_ONLY.to_string()),
                (None, false) => None,
            };
            let mut content: Vec<Value> = request
                .images
                .iter()
                .map(|image| {
                    json!({ "type": "image", "source": {
                        "type": "base64", "media_type": image.mime, "data": STANDARD.encode(&image.bytes)
                    } })
                })
                .collect();
            content.push(json!({ "type": "text", "text": request.prompt }));
            let mut body = json!({
                "model": model, "max_tokens": max_tokens,
                "messages": [{ "role": "user", "content": content }],
            });
            if let Some(system) = system {
                body["system"] = json!(system);
            }
            if let Some(temperature) = request.temperature {
                body["temperature"] = json!(temperature);
            }
            Call {
                url: format!("{base}/messages"),
                headers: vec![
                    ("x-api-key", key.to_string()),
                    ("anthropic-version", ANTHROPIC_VERSION.to_string()),
                ],
                body,
            }
        }
        Provider::Gemini => {
            let mut parts = vec![json!({ "text": request.prompt })];
            for image in &request.images {
                parts.push(json!({ "inline_data": { "mime_type": image.mime, "data": STANDARD.encode(&image.bytes) } }));
            }
            let mut generation = json!({ "maxOutputTokens": max_tokens });
            if let Some(temperature) = request.temperature {
                generation["temperature"] = json!(temperature);
            }
            if request.json {
                generation["responseMimeType"] = json!("application/json");
            }
            let mut body = json!({ "contents": [{ "role": "user", "parts": parts }], "generationConfig": generation });
            if let Some(system) = &request.system {
                body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
            }
            Call {
                url: format!("{base}/models/{model}:generateContent"),
                headers: vec![("x-goog-api-key", key.to_string())],
                body,
            }
        }
    }
}

/// Reads a successful answer; `Err` carries what is wrong with it (empty answer, blocked, ...).
pub fn read_text(provider: Provider, response: &Value) -> Result<Answer, String> {
    let (text, input_tokens, output_tokens) = match provider {
        Provider::OpenAi => {
            let choice = response.pointer("/choices/0");
            let text = choice
                .and_then(|c| c.pointer("/message/content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if text.is_empty() {
                let reason = choice
                    .and_then(|c| c.get("finish_reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("none");
                return Err(format!("OpenAI returned no text (finish reason: {reason})"));
            }
            (
                text.to_string(),
                tokens(response.pointer("/usage/prompt_tokens")),
                tokens(response.pointer("/usage/completion_tokens")),
            )
        }
        Provider::Claude => {
            let text: String = response
                .get("content")
                .and_then(Value::as_array)
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|b| b.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            if text.is_empty() {
                let reason = response
                    .get("stop_reason")
                    .and_then(Value::as_str)
                    .unwrap_or("none");
                return Err(format!("Claude returned no text (stop reason: {reason})"));
            }
            (
                text,
                tokens(response.pointer("/usage/input_tokens")),
                tokens(response.pointer("/usage/output_tokens")),
            )
        }
        Provider::Gemini => {
            let text: String = response
                .pointer("/candidates/0/content/parts")
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            if text.is_empty() {
                let reason = response
                    .pointer("/candidates/0/finishReason")
                    .or_else(|| response.pointer("/promptFeedback/blockReason"))
                    .and_then(Value::as_str)
                    .unwrap_or("none");
                return Err(format!("Gemini returned no text (reason: {reason})"));
            }
            (
                text,
                tokens(response.pointer("/usageMetadata/promptTokenCount")),
                tokens(response.pointer("/usageMetadata/candidatesTokenCount")),
            )
        }
    };
    Ok(Answer {
        text,
        input_tokens,
        output_tokens,
    })
}

/// The provider's own words for what went wrong, from an error response.
pub fn error_message(body: &Value, raw: &str) -> String {
    body.pointer("/error/message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| raw.chars().take(300).collect())
}

pub fn image_call(config: &AiConfig, request: &ImageRequest, model: &str, key: &str) -> Call {
    let base = config.base_url(Provider::OpenAi);
    let pick = |own: &Option<String>, configured: &str| {
        own.as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .or(Some(configured.trim()).filter(|v| !v.is_empty()))
            .map(str::to_owned)
    };
    let mut body = json!({ "model": model, "prompt": request.prompt, "n": request.count.unwrap_or(1).clamp(1, 10) });
    if let Some(size) = pick(&request.size, &config.image.size) {
        body["size"] = json!(size);
    }
    if let Some(quality) = pick(&request.quality, &config.image.quality) {
        body["quality"] = json!(quality);
    }
    if !config.image.output_format.trim().is_empty() {
        body["output_format"] = json!(config.image.output_format.trim());
    }
    Call {
        url: format!("{base}/images/generations"),
        headers: vec![("authorization", format!("Bearer {key}"))],
        body,
    }
}

/// Where each image is: inline (`b64_json`) or at a `url` that must be downloaded.
pub enum ImageSource {
    Inline(GeneratedImage),
    Url {
        url: String,
        revised_prompt: Option<String>,
    },
}

pub fn read_images(response: &Value, output_format: &str) -> Result<Vec<ImageSource>, String> {
    let mime = match output_format.trim() {
        "jpeg" | "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    let items = response
        .get("data")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty());
    let items = items.ok_or_else(|| "OpenAI returned no image".to_string())?;
    let mut images = Vec::new();
    for item in items {
        let revised_prompt = item
            .get("revised_prompt")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(encoded) = item.get("b64_json").and_then(Value::as_str) {
            let bytes = STANDARD
                .decode(encoded)
                .map_err(|error| format!("the image is not valid base64: {error}"))?;
            images.push(ImageSource::Inline(GeneratedImage {
                bytes,
                mime: mime.to_string(),
                revised_prompt,
            }));
        } else if let Some(url) = item.get("url").and_then(Value::as_str) {
            images.push(ImageSource::Url {
                url: url.to_string(),
                revised_prompt,
            });
        } else {
            return Err("OpenAI returned an image entry with no data".to_string());
        }
    }
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AiConfig {
        AiConfig::from_value(json!({
            "claude": { "api_key": "k" }, "image": { "size": "1024x1024", "quality": "high" }
        }))
        .unwrap()
    }

    fn request() -> Request {
        Request::new("Read this")
            .system("Be exact")
            .image("image/png", vec![1, 2, 3])
            .json()
            .temperature(0.1)
    }

    #[test]
    fn openai_gets_messages_an_image_url_and_json_mode() {
        let call = text_call(Provider::OpenAi, &config(), &request(), "m1", "sk");
        assert_eq!(call.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(
            call.headers,
            vec![("authorization", "Bearer sk".to_string())]
        );
        assert_eq!(call.body["messages"][0]["role"], "system");
        assert_eq!(
            call.body["messages"][1]["content"][1]["image_url"]["url"],
            "data:image/png;base64,AQID"
        );
        assert_eq!(call.body["response_format"]["type"], "json_object");
        assert_eq!(call.body["max_completion_tokens"], 2048);
    }

    #[test]
    fn claude_gets_images_first_and_the_json_instruction_in_the_system_prompt() {
        let call = text_call(Provider::Claude, &config(), &request(), "m2", "ak");
        assert_eq!(call.url, "https://api.anthropic.com/v1/messages");
        assert!(call.headers.contains(&("x-api-key", "ak".to_string())));
        assert!(call
            .headers
            .contains(&("anthropic-version", "2023-06-01".to_string())));
        let content = &call.body["messages"][0]["content"];
        assert_eq!(
            (content[0]["type"].as_str(), content[1]["type"].as_str()),
            (Some("image"), Some("text"))
        );
        assert_eq!(content[0]["source"]["data"], "AQID");
        assert!(call.body["system"]
            .as_str()
            .unwrap()
            .starts_with("Be exact\n\nReply with one valid JSON"));
        let plain = text_call(Provider::Claude, &config(), &Request::new("hi"), "m2", "ak");
        assert!(plain.body.get("system").is_none());
    }

    #[test]
    fn gemini_puts_the_model_in_the_url_and_asks_for_json_mime() {
        let call = text_call(Provider::Gemini, &config(), &request(), "gem-1", "gk");
        assert_eq!(
            call.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gem-1:generateContent"
        );
        assert_eq!(
            call.body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert_eq!(
            call.body["contents"][0]["parts"][1]["inline_data"]["mime_type"],
            "image/png"
        );
        assert_eq!(
            call.body["systemInstruction"]["parts"][0]["text"],
            "Be exact"
        );
    }

    #[test]
    fn answers_are_read_from_each_providers_shape() {
        let openai = json!({ "choices": [{ "message": { "content": "hi" } }], "usage": { "prompt_tokens": 5, "completion_tokens": 2 } });
        assert_eq!(
            read_text(Provider::OpenAi, &openai).unwrap(),
            Answer {
                text: "hi".into(),
                input_tokens: 5,
                output_tokens: 2
            }
        );
        let claude = json!({ "content": [{ "type": "text", "text": "a" }, { "type": "text", "text": "b" }], "usage": { "input_tokens": 7, "output_tokens": 1 } });
        assert_eq!(read_text(Provider::Claude, &claude).unwrap().text, "ab");
        let gemini = json!({ "candidates": [{ "content": { "parts": [{ "text": "x" }] } }], "usageMetadata": { "promptTokenCount": 3, "candidatesTokenCount": 4 } });
        let answer = read_text(Provider::Gemini, &gemini).unwrap();
        assert_eq!(
            (
                answer.text.as_str(),
                answer.input_tokens,
                answer.output_tokens
            ),
            ("x", 3, 4)
        );
    }

    #[test]
    fn an_empty_or_blocked_answer_is_an_error_with_the_reason() {
        let blocked = json!({ "promptFeedback": { "blockReason": "SAFETY" } });
        assert!(read_text(Provider::Gemini, &blocked)
            .unwrap_err()
            .contains("SAFETY"));
        let cut =
            json!({ "choices": [{ "message": { "content": "" }, "finish_reason": "length" }] });
        assert!(read_text(Provider::OpenAi, &cut)
            .unwrap_err()
            .contains("length"));
        assert!(read_text(
            Provider::Claude,
            &json!({ "stop_reason": "refusal", "content": [] })
        )
        .unwrap_err()
        .contains("refusal"));
    }

    #[test]
    fn error_messages_use_the_providers_words_or_the_raw_body() {
        assert_eq!(
            error_message(&json!({ "error": { "message": "bad key" } }), "x"),
            "bad key"
        );
        assert_eq!(
            error_message(&Value::Null, "gateway timeout"),
            "gateway timeout"
        );
    }

    #[test]
    fn image_calls_use_request_values_then_settings_and_answers_decode() {
        let call = image_call(
            &config(),
            &ImageRequest::new("a cat").size("1536x1024").count(2),
            "img-1",
            "sk",
        );
        assert_eq!(call.url, "https://api.openai.com/v1/images/generations");
        assert_eq!(
            (
                call.body["size"].as_str(),
                call.body["quality"].as_str(),
                call.body["n"].as_u64()
            ),
            (Some("1536x1024"), Some("high"), Some(2))
        );
        let response = json!({ "data": [{ "b64_json": "AQID", "revised_prompt": "a cute cat" }, { "url": "https://x/y.png" }] });
        let images = read_images(&response, "webp").unwrap();
        match &images[0] {
            ImageSource::Inline(image) => assert_eq!(
                (
                    image.bytes.as_slice(),
                    image.mime.as_str(),
                    image.revised_prompt.as_deref()
                ),
                (&[1u8, 2, 3][..], "image/webp", Some("a cute cat"))
            ),
            ImageSource::Url { .. } => panic!("expected inline"),
        }
        assert!(matches!(&images[1], ImageSource::Url { url, .. } if url == "https://x/y.png"));
        assert!(read_images(&json!({ "data": [] }), "").is_err());
    }
}

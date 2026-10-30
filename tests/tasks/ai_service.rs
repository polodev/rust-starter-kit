use std::sync::{Arc, Mutex};

use loco_rs::testing::prelude::*;
use module_ai_service::{ai_api_logs, AiConfig, AiService, ImageRequest, Provider, Request};
use sea_orm::{EntityTrait, QueryOrder};
use serde_json::{json, Value};
use serial_test::serial;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use rust_starter::app::App;

/// A web server that answers every request with the canned reply whose path fragment matches,
/// and remembers what it was sent as `(path, headers and body)`.
struct MockProvider {
    address: String,
    received: Arc<Mutex<Vec<(String, String)>>>,
}

async fn mock_provider(replies: Vec<(&'static str, u16, Value)>) -> MockProvider {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(Vec::new()));
    let remembered = received.clone();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            let mut buffer = [0u8; 8192];
            let (head_end, length) = loop {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break (0, 0);
                }
                data.extend_from_slice(&buffer[..read]);
                if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&data[..end]).to_lowercase();
                    let length = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (end + 4, length);
                }
            };
            while data.len() < head_end + length {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                data.extend_from_slice(&buffer[..read]);
            }
            let text = String::from_utf8_lossy(&data).to_string();
            let path = text.split_whitespace().nth(1).unwrap_or("").to_string();
            remembered.lock().unwrap().push((path.clone(), text));
            let (status, body) = replies
                .iter()
                .find(|(fragment, _, _)| path.contains(fragment))
                .map(|(_, status, body)| (*status, body.to_string()))
                .unwrap_or((
                    404,
                    json!({ "error": { "message": "no such route" } }).to_string(),
                ));
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    MockProvider { address, received }
}

fn config(base: &str, extra: Value) -> AiConfig {
    let mut value = json!({
        "default_provider": "claude",
        "claude": { "api_key": "claude-key", "base_url": base, "input_per_million": 3, "output_per_million": 15 },
        "openai": { "api_key": "openai-key", "base_url": base, "model": "gpt-test" },
        "gemini": { "api_key": "gemini-key", "base_url": base, "model": "gem-test" },
        "image": { "model": "image-test", "size": "1024x1024" },
    });
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    AiConfig::from_value(value).unwrap()
}

async fn logged(ctx: &loco_rs::app::AppContext) -> Vec<ai_api_logs::Model> {
    ai_api_logs::Entity::find()
        .order_by_asc(ai_api_logs::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn each_provider_is_called_in_its_own_way_and_every_call_is_logged() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    ai_api_logs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    let server = mock_provider(vec![
        ("/messages", 200, json!({ "content": [{ "type": "text", "text": "salam" }], "usage": { "input_tokens": 1000, "output_tokens": 200 } })),
        ("/chat/completions", 200, json!({ "choices": [{ "message": { "content": "hello" } }], "usage": { "prompt_tokens": 4, "completion_tokens": 2 } })),
        (":generateContent", 200, json!({ "candidates": [{ "content": { "parts": [{ "text": "namaste" }] } }], "usageMetadata": { "promptTokenCount": 6, "candidatesTokenCount": 3 } })),
    ])
    .await;
    let ai = AiService::with_config(ctx.db.clone(), config(&server.address, json!({}))).unwrap();

    // No provider named: the default (Claude), with the cost worked out from the prices in .env.
    let claude = ai
        .chat(
            Request::new("Say hi")
                .system("Be brief")
                .purpose("greeting"),
        )
        .await
        .unwrap();
    assert_eq!(
        (claude.text.as_str(), claude.provider, claude.model.as_str()),
        ("salam", Provider::Claude, Provider::Claude.default_model())
    );
    assert!(
        (claude.cost.unwrap() - 0.006).abs() < 1e-9,
        "{:?}",
        claude.cost
    );
    let openai = ai
        .chat(Request::new("Say hi").provider(Provider::OpenAi))
        .await
        .unwrap();
    assert_eq!(
        (
            openai.text.as_str(),
            openai.model.as_str(),
            openai.total_tokens()
        ),
        ("hello", "gpt-test", 6)
    );
    let gemini = ai
        .chat(
            Request::new("Say hi")
                .provider(Provider::Gemini)
                .model("gem-explicit"),
        )
        .await
        .unwrap();
    assert_eq!(gemini.text, "namaste");

    let received = server.received.lock().unwrap().clone();
    assert!(
        received[0]
            .1
            .to_lowercase()
            .contains("x-api-key: claude-key")
            && received[0].1.contains("anthropic-version")
    );
    assert!(received[1]
        .1
        .to_lowercase()
        .contains("authorization: bearer openai-key"));
    assert!(
        received[2].0.contains("gem-explicit:generateContent")
            && received[2]
                .1
                .to_lowercase()
                .contains("x-goog-api-key: gemini-key")
    );

    let rows = logged(&ctx).await;
    assert_eq!(rows.len(), 3);
    assert_eq!(
        (
            rows[0].provider.as_str(),
            rows[0].status.as_str(),
            rows[0].kind.as_str()
        ),
        ("claude", "success", "text")
    );
    assert_eq!(
        (
            rows[0].purpose.as_deref(),
            rows[0].input_tokens,
            rows[0].output_tokens
        ),
        (Some("greeting"), 1000, 200)
    );
    assert_eq!(
        (rows[0].prompt.as_deref(), rows[0].response.as_deref()),
        (Some("Say hi"), Some("salam"))
    );
    assert_eq!(rows[2].model, "gem-explicit");
}

#[tokio::test]
#[serial]
async fn failures_are_logged_with_the_providers_message_and_a_missing_key_is_clear() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    ai_api_logs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    let server = mock_provider(vec![
        ("/messages", 429, json!({ "error": { "message": "rate limit reached" } })),
        ("/chat/completions", 200, json!({ "choices": [{ "message": { "content": "" }, "finish_reason": "content_filter" }] })),
    ])
    .await;
    let ai = AiService::with_config(
        ctx.db.clone(),
        config(&server.address, json!({ "gemini": { "api_key": "" } })),
    )
    .unwrap();

    let limited = ai.chat(Request::new("hi")).await.unwrap_err().to_string();
    assert!(
        limited.contains("Claude") && limited.contains("rate limit reached"),
        "{limited}"
    );
    let empty = ai
        .chat(Request::new("hi").provider(Provider::OpenAi))
        .await
        .unwrap_err()
        .to_string();
    assert!(empty.contains("content_filter"), "{empty}");
    let no_key = ai
        .chat(Request::new("hi").provider(Provider::Gemini))
        .await
        .unwrap_err()
        .to_string();
    assert!(no_key.contains("GEMINI_API_KEY"), "{no_key}");

    let rows = logged(&ctx).await;
    assert_eq!(
        rows.len(),
        2,
        "a call that never left (no key) is not an AI API log row"
    );
    assert_eq!(
        (
            rows[0].status.as_str(),
            rows[0].http_status,
            rows[0].error.as_deref()
        ),
        ("failed", Some(429), Some("rate limit reached"))
    );
    assert!(rows[1].error.as_deref().unwrap().contains("content_filter"));

    // A server that is not there at all is a failed call too.
    let unreachable =
        AiService::with_config(ctx.db.clone(), config("http://127.0.0.1:1", json!({}))).unwrap();
    assert!(unreachable.chat(Request::new("hi")).await.is_err());
    let rows = logged(&ctx).await;
    assert_eq!(
        (rows.len(), rows[2].status.as_str(), rows[2].http_status),
        (3, "failed", None)
    );
}

#[tokio::test]
#[serial]
async fn a_picture_is_read_into_json_and_a_sensitive_call_keeps_no_content() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    ai_api_logs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    let passport = "```json\n{\"surname\":\"KHAN\",\"number\":\"A1234567\"}\n```";
    let server = mock_provider(vec![("/messages", 200, json!({ "content": [{ "type": "text", "text": passport }], "usage": { "input_tokens": 50, "output_tokens": 20 } }))]).await;
    let ai = AiService::with_config(ctx.db.clone(), config(&server.address, json!({}))).unwrap();

    let (data, reply) = ai
        .json(
            Request::new("Read this passport")
                .image("image/jpeg", vec![1, 2, 3])
                .purpose("passport-scan")
                .sensitive(),
        )
        .await
        .unwrap();
    assert_eq!(data, json!({ "surname": "KHAN", "number": "A1234567" }));
    assert_eq!(reply.provider, Provider::Claude);
    let sent = server.received.lock().unwrap()[0].1.clone();
    assert!(
        sent.contains(r#""type":"image""#) && sent.contains("AQID") && sent.contains("valid JSON")
    );

    let row = &logged(&ctx).await[0];
    assert_eq!(
        (row.kind.as_str(), row.image_count, row.sensitive),
        ("vision", 1, true)
    );
    assert!(
        row.prompt.is_none() && row.response.is_none(),
        "a sensitive call stores no content"
    );
    let dump = serde_json::to_string(row).unwrap();
    assert!(!dump.contains("KHAN") && !dump.contains("A1234567") && !dump.contains("AQID"));

    // With content logging switched off in .env nothing is kept either, but sizes and status are.
    let quiet = AiService::with_config(
        ctx.db.clone(),
        config(&server.address, json!({ "log_content": "false" })),
    )
    .unwrap();
    quiet.chat(Request::new("secret question")).await.unwrap();
    let row = logged(&ctx).await.pop().unwrap();
    assert!(row.prompt.is_none() && row.response.is_none() && row.input_tokens == 50);

    // A reply that is not JSON is an error, and the call is still in the AI API logs.
    let chatty = mock_provider(vec![(
        "/messages",
        200,
        json!({ "content": [{ "type": "text", "text": "I cannot read it" }] }),
    )])
    .await;
    let ai = AiService::with_config(ctx.db.clone(), config(&chatty.address, json!({}))).unwrap();
    let error = ai.json(Request::new("Read")).await.unwrap_err().to_string();
    assert!(
        error.contains("did not return valid JSON") && error.contains("AI API logs"),
        "{error}"
    );
}

#[tokio::test]
#[serial]
async fn images_are_generated_with_openai_and_logged() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    ai_api_logs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    let server = mock_provider(vec![(
        "/images/generations",
        200,
        json!({ "data": [{ "b64_json": "AQID", "revised_prompt": "a bicycle, red" }] }),
    )])
    .await;
    let ai = AiService::with_config(ctx.db.clone(), config(&server.address, json!({}))).unwrap();

    let images = ai
        .generate_image(
            ImageRequest::new("a red bicycle")
                .size("1536x1024")
                .purpose("blog-cover"),
        )
        .await
        .unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(
        (
            images[0].bytes.as_slice(),
            images[0].extension(),
            images[0].revised_prompt.as_deref()
        ),
        (&[1u8, 2, 3][..], "png", Some("a bicycle, red"))
    );
    let sent = server.received.lock().unwrap()[0].1.clone();
    assert!(sent.contains(r#""model":"image-test""#) && sent.contains("1536x1024"));
    let row = &logged(&ctx).await[0];
    assert_eq!(
        (
            row.kind.as_str(),
            row.provider.as_str(),
            row.purpose.as_deref()
        ),
        ("image", "openai", Some("blog-cover"))
    );

    // Image generation needs an OpenAI key.
    let keyless = AiService::with_config(
        ctx.db.clone(),
        config(&server.address, json!({ "openai": { "api_key": "" } })),
    )
    .unwrap();
    assert!(keyless
        .generate_image(ImageRequest::new("x"))
        .await
        .unwrap_err()
        .to_string()
        .contains("OPENAI_API_KEY"));
}

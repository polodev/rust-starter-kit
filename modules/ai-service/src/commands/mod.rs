use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Tasks, Vars},
    Error, Result,
};

use crate::{AiService, ImageRequest, Provider, Request};

fn text_argument(vars: &Vars, key: &str) -> Option<String> {
    vars.cli_arg(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// `ai-service:ask`: sends one prompt to a provider and prints the answer, the tokens and the cost.
pub struct AskArtificialIntelligence;

#[async_trait]
impl Task for AskArtificialIntelligence {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "ai-service:ask".to_string(),
            detail: "Send a prompt to an AI provider (default from .env) and print the reply.\nUsage: cargo run -- task ai-service:ask prompt:\"Say hi in Bangla\" provider:claude model:claude-sonnet-5-5".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let prompt = text_argument(vars, "prompt")
            .ok_or_else(|| Error::string("prompt:\"...\" is required"))?;
        let mut request = Request::new(&prompt).purpose("ai-service:ask");
        if let Some(name) = text_argument(vars, "provider") {
            request = request.provider(
                Provider::parse(&name)
                    .ok_or_else(|| Error::string("provider must be claude, openai or gemini"))?,
            );
        }
        if let Some(model) = text_argument(vars, "model") {
            request = request.model(&model);
        }
        let reply = AiService::new(ctx)?.chat(request).await?;
        println!("{}", reply.text);
        println!(
            "\n[{} {} | {} input + {} output tokens{}]",
            reply.provider.label(),
            reply.model,
            reply.input_tokens,
            reply.output_tokens,
            reply
                .cost
                .map(|cost| format!(" | about ${cost:.4}"))
                .unwrap_or_default()
        );
        Ok(())
    }
}

/// `ai-service:generate-image`: makes a picture with OpenAI and saves it to a file.
pub struct GenerateImage;

#[async_trait]
impl Task for GenerateImage {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "ai-service:generate-image".to_string(),
            detail: "Generate an image with OpenAI and save it.\nUsage: cargo run -- task ai-service:generate-image prompt:\"a red bicycle\" size:1024x1024 output:storage/ai-image".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let prompt = text_argument(vars, "prompt")
            .ok_or_else(|| Error::string("prompt:\"...\" is required"))?;
        let mut request = ImageRequest::new(&prompt).purpose("ai-service:generate-image");
        if let Some(size) = text_argument(vars, "size") {
            request = request.size(&size);
        }
        let output =
            text_argument(vars, "output").unwrap_or_else(|| "storage/ai-image".to_string());
        let images = AiService::new(ctx)?.generate_image(request).await?;
        if let Some(parent) = std::path::Path::new(&output)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        for (position, image) in images.iter().enumerate() {
            let path = format!("{output}-{}.{}", position + 1, image.extension());
            std::fs::write(&path, &image.bytes)?;
            println!("saved {path} ({} bytes)", image.bytes.len());
        }
        Ok(())
    }
}

/// `ai-service:status`: which providers have a key and which models would be used. Sends nothing.
pub struct ShowStatus;

#[async_trait]
impl Task for ShowStatus {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "ai-service:status".to_string(),
            detail: "Show the default AI provider and, per provider, whether a key is set and which model is used. No key is printed.".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let config = crate::AiConfig::load(ctx)?;
        match config.default_provider() {
            Ok(provider) => println!(
                "default provider: {} ({})",
                provider.label(),
                provider.name()
            ),
            Err(error) => println!("default provider: none ({error})"),
        }
        for provider in Provider::ALL {
            println!(
                "{:<7} key: {:<8} model: {}",
                provider.label(),
                if config.has_key(provider) {
                    "set"
                } else {
                    "missing"
                },
                config.model(provider, None)
            );
        }
        println!(
            "image   key: {:<8} model: {}",
            if config.has_key(Provider::OpenAi) {
                "set"
            } else {
                "missing"
            },
            config.image_model()
        );
        println!(
            "AI API logs keep prompts and replies: {}",
            if config.log_content { "yes" } else { "no" }
        );
        Ok(())
    }
}

/// `ai-service:purge-ai-api-logs`: deletes old rows of the AI API logs. Scheduled daily.
pub struct PurgeAiApiLogs;

#[async_trait]
impl Task for PurgeAiApiLogs {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "ai-service:purge-ai-api-logs".to_string(),
            detail: "Delete AI API logs older than N days (default 30).\nUsage: cargo run -- task ai-service:purge-ai-api-logs days:7".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let days = match vars.cli_arg("days") {
            Ok(text) => text
                .parse::<i64>()
                .ok()
                .filter(|days| *days > 0)
                .ok_or_else(|| Error::string("days must be a positive whole number"))?,
            Err(_) => crate::RETENTION_DAYS,
        };
        let removed = crate::controllers::remove_ai_api_logs_older_than(&ctx.db, days).await?;
        println!("removed {removed} AI API log(s) older than {days} day(s)");
        Ok(())
    }
}

pub fn register(tasks: &mut Tasks) {
    tasks.register(AskArtificialIntelligence);
    tasks.register(GenerateImage);
    tasks.register(ShowStatus);
    tasks.register(PurgeAiApiLogs);
}

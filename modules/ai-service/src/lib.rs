//! One wrapper for AI providers. Text (and reading pictures) goes to Claude, OpenAI or Gemini; image
//! generation goes to OpenAI. Keys, models and the default provider come from `.env` through
//! `settings.ai` (see [`config`]). Other modules call [`AiService`]; every call is written to the AI API logs
//! (`ai_api_logs`), which staff read and clean at `/admin/ai-api-logs` (`ai_api_logs.view`, `ai_api_logs.manage`).

mod commands;
mod config;
mod controllers;
mod entities;
mod migrations;
mod providers;
mod service;
mod types;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module, ScheduledJob};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use config::{AiConfig, ImageConfig, ProviderConfig};
pub use entities::ai_api_logs;
pub use service::{extract_json, AiService};
pub use types::{GeneratedImage, ImageInput, ImageRequest, Provider, Reply, Request};

/// AI API logs older than this are removed by `ai-service:purge-ai-api-logs`.
pub const RETENTION_DAYS: i64 = 30;

pub struct AiServiceModule;

#[async_trait]
impl Module for AiServiceModule {
    fn name(&self) -> &'static str {
        "ai-service"
    }

    fn routes(&self) -> Vec<Routes> {
        controllers::routes()
    }

    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }

    fn schedule(&self) -> Vec<ScheduledJob> {
        vec![ScheduledJob::command("ai-service:purge-ai-api-logs")
            .daily_at(3, 50)
            .tag("maintenance")]
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "admin/ai_api_logs_index.html",
                include_str!("../templates/admin/ai_api_logs_index.html"),
            ),
            (
                "admin/ai_api_log_show.html",
                include_str!("../templates/admin/ai_api_log_show.html"),
            ),
        ]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![MenuItem {
            section: "System",
            label: "AI API logs",
            href: "/admin/ai-api-logs",
            permission: "ai_api_logs.view",
            icon: "fa-solid fa-robot",
            order: 5,
            match_children: true,
        }]
    }
}

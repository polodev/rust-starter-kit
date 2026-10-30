use loco_rs::cli;
use migration::Migrator;
use rust_starter::app::App;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    // Read `.env` (git-ignored) so `get_env(...)` in config/*.yaml sees it. Variables already set in the
    // real environment win, which is how production is configured. A missing `.env` is fine.
    dotenvy::dotenv().ok();
    cli::main::<App, Migrator>().await
}

use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Tasks, Vars},
    Error, Result,
};

use crate::{Invoice, PdfOptions};

// mods-inject (do not remove)

/// `pdf:sample`: writes a sample invoice PDF, to see that Chrome and the fonts work.
pub struct Sample;

#[async_trait]
impl Task for Sample {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "pdf:sample".to_string(),
            detail: "Write a sample invoice PDF.\nUsage: cargo run -- task pdf:sample out:/tmp/invoice.pdf".to_string(),
        }
    }

    async fn run(&self, _ctx: &AppContext, vars: &Vars) -> Result<()> {
        let out = vars.cli_arg("out").unwrap_or("sample-invoice.pdf");
        let bytes = Invoice::sample("customer@example.com")
            .pdf(&PdfOptions::from_env())
            .await?;
        std::fs::write(out, &bytes).map_err(|error| Error::string(&error.to_string()))?;
        println!("wrote {out} ({} KB)", bytes.len().div_ceil(1024));
        Ok(())
    }
}

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(Sample);
    // tasks-inject (do not remove)
}

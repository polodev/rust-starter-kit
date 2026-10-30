use loco_rs::prelude::*;
use module_queue_watcher::{track, JobInfo};
use serde::{Deserialize, Serialize};

pub struct DownloadWorker {
    pub ctx: AppContext,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct DownloadWorkerArgs {
    pub user_guid: String,
}

#[async_trait]
impl BackgroundWorker<DownloadWorkerArgs> for DownloadWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
    async fn perform(&self, args: DownloadWorkerArgs) -> Result<()> {
        // Recording the run makes it visible in /admin/queue-watcher (status, time taken, any error).
        let info = JobInfo::new("DownloadWorker")
            .queue("downloads")
            .class("rust_starter::workers::DownloadWorker")
            .payload(serde_json::to_value(&args).unwrap_or_default());
        track(&self.ctx, info, async {
            // TODO: Some actual work goes here...
            Ok(())
        })
        .await
    }
}

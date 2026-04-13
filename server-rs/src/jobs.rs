use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, instrument};

#[derive(Debug, Clone)]
pub enum Job {
    ExtractMetadata { asset_id: String },
    GenerateThumbnail { asset_id: String },
    TranscodeVideo { asset_id: String },
    // More jobs can be added here
}

pub struct JobQueue {
    pub sender: mpsc::Sender<Job>,
}

impl JobQueue {
    pub fn new() -> (Self, mpsc::Receiver<Job>) {
        // Channel size dictates how many jobs can be queued in memory before `send().await` pushes back.
        let (sender, receiver) = mpsc::channel(1024);
        (Self { sender }, receiver)
    }
}

pub async fn run_worker(mut receiver: mpsc::Receiver<Job>) {
    info!("Background job worker started");
    while let Some(job) = receiver.recv().await {
        process_job(job).await;
    }
    info!("Background job worker exited");
}

#[instrument]
async fn process_job(job: Job) {
    // In a real implementation this would fetch from DB, run ffmpeg/exiftool, update DB.
    match job {
        Job::ExtractMetadata { asset_id } => {
            info!("Processing metadata for asset {}", asset_id);
            // Simulate work
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        Job::GenerateThumbnail { asset_id } => {
            info!("Generating thumbnail for asset {}", asset_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        }
        Job::TranscodeVideo { asset_id } => {
            info!("Transcoding video for asset {}", asset_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
        }
    }
}

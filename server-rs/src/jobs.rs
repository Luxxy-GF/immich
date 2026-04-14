use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tracing::info;

use crate::{controllers::asset, error::AppError, AppState};

#[derive(Debug, Clone)]
pub enum Job {
    ExtractMetadata { id: String, asset_id: String },
    GenerateThumbnail { id: String, asset_id: String },
    TranscodeVideo { id: String, asset_id: String },
    SmartSearch { id: String, asset_id: String },
    DetectFaces { id: String, asset_id: String },
    Ocr { id: String, asset_id: String },
}

pub struct JobQueue {
    pub sender: mpsc::Sender<Job>,
    pub(crate) store: Arc<Mutex<JobStore>>,
}

impl JobQueue {
    pub fn new() -> (Self, mpsc::Receiver<Job>) {
        // Channel size dictates how many jobs can be queued in memory before `send().await` pushes back.
        let (sender, receiver) = mpsc::channel(1024);
        let store = Arc::new(Mutex::new(JobStore::default()));
        (Self { sender, store }, receiver)
    }

    pub async fn enqueue(&self, job: Job) -> Result<(), AppError> {
        let record = JobRecord::from_job(&job);
        {
            let mut store = self.store.lock().await;
            store.jobs.push(record);
        }
        self.sender
            .send(job)
            .await
            .map_err(|e| AppError::InternalServerError(e.into()))?;
        Ok(())
    }

    pub async fn set_paused(&self, queue: &str, paused: bool) {
        let mut store = self.store.lock().await;
        if paused {
            store.paused.insert(queue.to_string());
        } else {
            store.paused.remove(queue);
        }
    }

    pub async fn is_paused(&self, queue: &str) -> bool {
        self.store.lock().await.paused.contains(queue)
    }

    pub async fn clear_queue(&self, queue: &str) {
        let mut store = self.store.lock().await;
        store.jobs.retain(|job| job.queue != queue || job.status == "active");
    }

    pub async fn queue_stats(&self, queue: &str) -> QueueStats {
        let store = self.store.lock().await;
        let mut stats = QueueStats::default();
        for job in store.jobs.iter().filter(|job| job.queue == queue) {
            match job.status.as_str() {
                "active" => stats.active += 1,
                "completed" => stats.completed += 1,
                "failed" => stats.failed += 1,
                "delayed" => stats.delayed += 1,
                "paused" => stats.paused += 1,
                _ => stats.waiting += 1,
            }
        }
        if store.paused.contains(queue) {
            stats.is_paused = true;
            stats.paused = stats.paused.max(1);
        }
        stats
    }

    pub async fn jobs_for_queue(&self, queue: &str) -> Vec<JobRecord> {
        let store = self.store.lock().await;
        store
            .jobs
            .iter()
            .filter(|job| job.queue == queue)
            .cloned()
            .collect()
    }
}

pub async fn run_worker(receiver: Arc<Mutex<mpsc::Receiver<Job>>>, state: AppState, worker_id: usize) {
    info!("Background job worker started ({worker_id})");
    loop {
        let job = {
            let mut receiver = receiver.lock().await;
            receiver.recv().await
        };

        let Some(job) = job else {
            break;
        };
        process_job(job, &state).await;
    }
    info!("Background job worker exited ({worker_id})");
}

async fn process_job(job: Job, state: &AppState) {
    let job_id = job.id().to_string();
    update_job_status(state, &job_id, "active", None).await;

    let result = asset::run_media_job(state, &job).await;
    match result {
        Ok(()) => update_job_status(state, &job_id, "completed", None).await,
        Err(_err) => update_job_status(state, &job_id, "failed", Some("job failed".to_string())).await,
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub active: i64,
    pub completed: i64,
    pub delayed: i64,
    pub failed: i64,
    pub paused: i64,
    pub waiting: i64,
    pub is_paused: bool,
}

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: String,
    pub name: String,
    pub queue: String,
    pub status: String,
    pub data: serde_json::Value,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub failed_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Default)]
struct JobStore {
    jobs: Vec<JobRecord>,
    paused: HashSet<String>,
}

impl JobRecord {
    fn from_job(job: &Job) -> Self {
        let (queue, name, asset_id) = job_descriptor(job);
        Self {
            id: job.id().to_string(),
            name: name.to_string(),
            queue: queue.to_string(),
            status: "waiting".to_string(),
            data: serde_json::json!({ "assetId": asset_id }),
            created_at: chrono::Utc::now().timestamp_millis(),
            started_at: None,
            completed_at: None,
            failed_at: None,
            error: None,
        }
    }
}

impl Job {
    pub fn id(&self) -> &str {
        match self {
            Job::ExtractMetadata { id, .. } => id,
            Job::GenerateThumbnail { id, .. } => id,
            Job::TranscodeVideo { id, .. } => id,
            Job::SmartSearch { id, .. } => id,
            Job::DetectFaces { id, .. } => id,
            Job::Ocr { id, .. } => id,
        }
    }
}

fn job_descriptor(job: &Job) -> (&'static str, &'static str, &str) {
    match job {
        Job::ExtractMetadata { asset_id, .. } => ("metadataExtraction", "AssetExtractMetadata", asset_id),
        Job::GenerateThumbnail { asset_id, .. } => ("thumbnailGeneration", "AssetGenerateThumbnails", asset_id),
        Job::TranscodeVideo { asset_id, .. } => ("videoConversion", "AssetTranscodeVideo", asset_id),
        Job::SmartSearch { asset_id, .. } => ("smartSearch", "AssetSmartSearch", asset_id),
        Job::DetectFaces { asset_id, .. } => ("faceDetection", "AssetDetectFaces", asset_id),
        Job::Ocr { asset_id, .. } => ("ocr", "AssetOcr", asset_id),
    }
}

async fn update_job_status(state: &AppState, job_id: &str, status: &str, error: Option<String>) {
    let mut store = state.job_queue.store.lock().await;
    if let Some(job) = store.jobs.iter_mut().find(|job| job.id == job_id) {
        job.status = status.to_string();
        let now = chrono::Utc::now().timestamp_millis();
        match status {
            "active" => job.started_at = Some(now),
            "completed" => job.completed_at = Some(now),
            "failed" => {
                job.failed_at = Some(now);
                job.error = error;
            }
            _ => {}
        }
    }
}

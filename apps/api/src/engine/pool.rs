//! A bounded pool of worker processes, one per project, plus the tool catalogue with policy.
//! Workers are spawned on demand and the least-recently-used is stopped when the pool is full.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{Mutex, OnceCell};

use super::policy::{ToolInfo, from_catalog};
use super::worker::{Worker, WorkerLaunch};

struct Entry {
    worker: Arc<Worker>,
    deep: bool,
    last_used: Instant,
}

pub struct EnginePool {
    launch: WorkerLaunch,
    max_workers: usize,
    workers: Mutex<HashMap<String, Entry>>,
    catalog: OnceCell<Arc<HashMap<String, ToolInfo>>>,
    catalog_json: OnceCell<Arc<Vec<serde_json::Value>>>,
}

impl EnginePool {
    pub fn new(launch: WorkerLaunch, max_workers: usize) -> Self {
        EnginePool { launch, max_workers: max_workers.max(1), workers: Mutex::new(HashMap::new()), catalog: OnceCell::new(), catalog_json: OnceCell::new() }
    }

    pub fn launch(&self) -> &WorkerLaunch {
        &self.launch
    }

    /// The tool catalogue (id → info with policy), produced once and cached.
    pub async fn catalog(&self) -> Result<Arc<HashMap<String, ToolInfo>>, super::WorkerError> {
        self.ensure_catalog().await?;
        Ok(self.catalog.get().cloned().unwrap_or_default())
    }

    /// The catalogue as the JSON clients see (`GET /tools`), with policy added per entry.
    pub async fn catalog_json(&self) -> Result<Arc<Vec<serde_json::Value>>, super::WorkerError> {
        self.ensure_catalog().await?;
        Ok(self.catalog_json.get().cloned().unwrap_or_default())
    }

    async fn ensure_catalog(&self) -> Result<(), super::WorkerError> {
        if self.catalog.initialized() {
            return Ok(());
        }
        let raw = super::worker::global_catalog(&self.launch).await?;
        let mut map = HashMap::new();
        let mut json = Vec::with_capacity(raw.len());
        for entry in &raw {
            if let Some(info) = from_catalog(entry) {
                let mut e = entry.clone();
                if let Some(obj) = e.as_object_mut() {
                    obj.insert("policy".into(), serde_json::json!(info.policy.as_str()));
                }
                json.push(e);
                map.insert(info.id.clone(), info);
            }
        }
        let _ = self.catalog.set(Arc::new(map));
        let _ = self.catalog_json.set(Arc::new(json));
        Ok(())
    }

    /// A worker for this project, spawning one if needed and matching Deep mode.
    pub async fn worker_for(&self, workspace_id: &str, project_id: &str, deep: bool) -> Result<Arc<Worker>, super::WorkerError> {
        let mut workers = self.workers.lock().await;
        if let Some(entry) = workers.get_mut(project_id) {
            if entry.deep != deep {
                // Match the project's current mode on the live worker.
                let _ = entry.worker.set_deep(deep).await;
                entry.deep = deep;
            }
            entry.last_used = Instant::now();
            return Ok(entry.worker.clone());
        }
        if workers.len() >= self.max_workers
            && let Some(oldest) = workers.iter().min_by_key(|(_, e)| e.last_used).map(|(k, _)| k.clone())
            && let Some(entry) = workers.remove(&oldest)
        {
            let w = entry.worker.clone();
            tokio::spawn(async move { w.shutdown().await });
        }
        let worker = Arc::new(Worker::spawn(&self.launch, workspace_id, project_id, deep).await?);
        workers.insert(project_id.to_string(), Entry { worker: worker.clone(), deep, last_used: Instant::now() });
        Ok(worker)
    }

    /// Stops a project's worker (e.g. the project was deleted).
    pub async fn evict(&self, project_id: &str) {
        let entry = self.workers.lock().await.remove(project_id);
        if let Some(entry) = entry {
            entry.worker.shutdown().await;
        }
    }

    pub async fn shutdown_all(&self) {
        let mut workers = self.workers.lock().await;
        for (_, entry) in workers.drain() {
            entry.worker.shutdown().await;
        }
    }
}

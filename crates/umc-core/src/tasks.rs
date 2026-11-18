//! Persistent task state, cancellation, progress events, and bounded execution.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc, watch};
use umc_api::{TaskEvent, TaskHandle, TaskInfo, TaskStatus};
use uuid::Uuid;

const MAX_ACTIVE_OPERATIONS: usize = 2;
const MAX_ACTIVE_DOWNLOADS: usize = 32;
const MAX_TASK_SECONDS: u64 = 6 * 60 * 60;
const CANCEL_GRACE_SECONDS: u64 = 10;

#[derive(Clone)]
pub(super) struct TaskManager {
    root: PathBuf,
    active: Arc<Mutex<std::collections::HashMap<String, watch::Sender<bool>>>>,
    operations: Arc<Semaphore>,
    downloads: Arc<Semaphore>,
}

#[derive(Clone)]
pub(super) struct TaskContext {
    cancelled: watch::Receiver<bool>,
    updates: mpsc::UnboundedSender<(String, Value)>,
    downloads: Arc<Semaphore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskRecord {
    info: TaskInfo,
}

impl TaskManager {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            active: Arc::new(Mutex::new(std::collections::HashMap::new())),
            operations: Arc::new(Semaphore::new(MAX_ACTIVE_OPERATIONS)),
            downloads: Arc::new(Semaphore::new(MAX_ACTIVE_DOWNLOADS)),
        }
    }

    pub async fn spawn<F, Fut>(
        &self,
        operation: &str,
        timeout_seconds: u64,
        run: F,
    ) -> Result<TaskHandle, String>
    where
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        if !(1..=MAX_TASK_SECONDS).contains(&timeout_seconds) {
            return Err(format!(
                "timeout_seconds must be between 1 and {MAX_TASK_SECONDS}"
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = super::engine::timestamp_rfc3339();
        let mut record = TaskRecord {
            info: TaskInfo {
                task_id: id.clone(),
                operation: operation.to_owned(),
                status: TaskStatus::Queued,
                created_at: now.clone(),
                updated_at: now,
                timeout_seconds,
                sequence: 0,
                progress: Value::Null,
                result: None,
                error: None,
            },
        };
        self.save(&record).await?;
        self.event(&mut record, "task.queued", serde_json::json!({}))
            .await?;

        let (cancel_sender, cancel_receiver) = watch::channel(false);
        let (update_sender, update_receiver) = mpsc::unbounded_channel();
        self.active
            .lock()
            .map_err(|_| "Task manager lock poisoned".to_string())?
            .insert(id.clone(), cancel_sender.clone());
        let context = TaskContext {
            cancelled: cancel_receiver.clone(),
            updates: update_sender,
            downloads: self.downloads.clone(),
        };
        let manager = self.clone();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_seconds);
        tokio::spawn(async move {
            manager
                .execute(
                    record,
                    run,
                    context,
                    cancel_sender,
                    cancel_receiver,
                    update_receiver,
                    timeout_seconds,
                    deadline,
                )
                .await;
        });
        Ok(TaskHandle {
            task_id: id,
            status: TaskStatus::Queued,
        })
    }

    async fn execute<F, Fut>(
        &self,
        mut record: TaskRecord,
        run: F,
        context: TaskContext,
        cancel_sender: watch::Sender<bool>,
        mut cancel_receiver: watch::Receiver<bool>,
        mut updates: mpsc::UnboundedReceiver<(String, Value)>,
        timeout_seconds: u64,
        deadline: tokio::time::Instant,
    ) where
        F: FnOnce(TaskContext) -> Fut,
        Fut: Future<Output = Result<Value, String>>,
    {
        let mut operation_cancel = cancel_receiver.clone();
        let queue_timeout = tokio::time::sleep_until(deadline);
        tokio::pin!(queue_timeout);
        let permit = tokio::select! {
            permit = self.operations.clone().acquire_owned() => permit.map_err(|_| "Task limiter closed"),
            _ = operation_cancel.changed() => Err("Task cancelled while queued"),
            _ = &mut queue_timeout => Err("Task timed out while queued"),
        };
        let _permit = match permit {
            Ok(permit) => permit,
            Err(error) => {
                let status = if error.contains("timed out") {
                    TaskStatus::Failed
                } else {
                    TaskStatus::Cancelled
                };
                self.finish(&mut record, status, None, Some(error.into()))
                    .await;
                self.remove_active(&record.info.task_id);
                return;
            }
        };
        if *cancel_receiver.borrow() {
            self.finish(
                &mut record,
                TaskStatus::Cancelled,
                None,
                Some("Task cancelled while queued".into()),
            )
            .await;
            self.remove_active(&record.info.task_id);
            return;
        }
        record.info.status = TaskStatus::Running;
        if self
            .event(&mut record, "task.started", serde_json::json!({}))
            .await
            .is_err()
        {
            self.remove_active(&record.info.task_id);
            return;
        }

        let future = run(context);
        tokio::pin!(future);
        let timeout = tokio::time::sleep_until(deadline);
        tokio::pin!(timeout);
        let mut timed_out = false;
        let mut cancel_deadline = None;
        let outcome = loop {
            tokio::select! {
                result = &mut future => break result,
                update = updates.recv() => {
                    if let Some((event, data)) = update {
                        if event == "download.progress" || event == "install.progress" {
                            record.info.progress = data.clone();
                        }
                        if self.event(&mut record, &event, data).await.is_err() {
                            let _ = cancel_sender.send(true);
                            break Err("Could not persist task progress".into());
                        }
                    }
                }
                changed = cancel_receiver.changed(), if cancel_deadline.is_none() => {
                    if changed.is_ok() && *cancel_receiver.borrow() {
                        cancel_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(CANCEL_GRACE_SECONDS));
                    }
                }
                _ = &mut timeout, if !timed_out => {
                    timed_out = true;
                    let _ = cancel_sender.send(true);
                    cancel_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(CANCEL_GRACE_SECONDS));
                    let _ = self.event(&mut record, "task.timeout", serde_json::json!({"timeout_seconds":timeout_seconds})).await;
                }
                _ = async {
                    if let Some(deadline) = cancel_deadline { tokio::time::sleep_until(deadline).await; }
                    else { std::future::pending::<()>().await; }
                } => {
                    break Err(if timed_out { "Task timed out and did not stop cleanly".into() } else { "Task cancellation timed out".into() });
                }
            }
        };
        match outcome {
            Ok(result) if !timed_out && !*cancel_receiver.borrow() => {
                self.finish(&mut record, TaskStatus::Succeeded, Some(result), None)
                    .await
            }
            Ok(_) if timed_out => {
                self.finish(
                    &mut record,
                    TaskStatus::Failed,
                    None,
                    Some("Task timed out".into()),
                )
                .await
            }
            Ok(_) => {
                self.finish(
                    &mut record,
                    TaskStatus::Cancelled,
                    None,
                    Some("Task cancelled by client".into()),
                )
                .await
            }
            Err(error) if timed_out => {
                self.finish(
                    &mut record,
                    TaskStatus::Failed,
                    None,
                    Some(format!("Task timed out: {error}")),
                )
                .await
            }
            Err(error) if *cancel_receiver.borrow() => {
                self.finish(&mut record, TaskStatus::Cancelled, None, Some(error))
                    .await
            }
            Err(error) => {
                self.finish(&mut record, TaskStatus::Failed, None, Some(error))
                    .await
            }
        }
        self.remove_active(&record.info.task_id);
    }

    async fn finish(
        &self,
        record: &mut TaskRecord,
        status: TaskStatus,
        result: Option<Value>,
        error: Option<String>,
    ) {
        record.info.status = status.clone();
        record.info.result = result;
        record.info.error = error.clone();
        let (event, data) = match status {
            TaskStatus::Succeeded => (
                "task.completed",
                serde_json::json!({"result":record.info.result}),
            ),
            TaskStatus::Cancelled => ("task.cancelled", serde_json::json!({"error":error})),
            _ => ("task.failed", serde_json::json!({"error":error})),
        };
        let _ = self.event(record, event, data).await;
    }

    fn remove_active(&self, id: &str) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(id);
        }
    }

    async fn event(&self, record: &mut TaskRecord, name: &str, data: Value) -> Result<(), String> {
        record.info.sequence += 1;
        let event = TaskEvent {
            task_id: record.info.task_id.clone(),
            sequence: record.info.sequence,
            timestamp: super::engine::timestamp_rfc3339(),
            event: name.into(),
            data,
        };
        let events_dir = self.root.join("tasks/events");
        tokio::fs::create_dir_all(&events_dir)
            .await
            .map_err(|e| e.to_string())?;
        super::engine::set_private_dir(&events_dir).await?;
        let path = events_dir.join(format!("{}.jsonl", record.info.task_id));
        super::engine::ensure_no_symlink(&self.root, &path).await?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| e.to_string())?;
        let mut line = serde_json::to_vec(&event).map_err(|e| e.to_string())?;
        line.push(b'\n');
        use tokio::io::AsyncWriteExt;
        file.write_all(&line).await.map_err(|e| e.to_string())?;
        file.flush().await.map_err(|e| e.to_string())?;
        super::engine::set_private_file(&path)?;
        record.info.updated_at = event.timestamp.clone();
        self.save(record).await
    }

    async fn save(&self, record: &TaskRecord) -> Result<(), String> {
        let dir = self.root.join("tasks");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
        super::engine::set_private_dir(&dir).await?;
        let path = dir.join(format!("{}.json", record.info.task_id));
        super::engine::ensure_no_symlink(&self.root, &path).await?;
        let temp = dir.join(format!("{}.{}.tmp", record.info.task_id, Uuid::new_v4()));
        tokio::fs::write(
            &temp,
            serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
        super::engine::set_private_file(&temp)?;
        if cfg!(windows) && tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| e.to_string())?;
        }
        tokio::fs::rename(temp, path)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get(&self, id: &str) -> Result<TaskInfo, String> {
        let mut record = self.read(id).await?;
        let active = self
            .active
            .lock()
            .map_err(|_| "Task manager lock poisoned".to_string())?
            .contains_key(id);
        if !active && matches!(record.info.status, TaskStatus::Queued | TaskStatus::Running) {
            record.info.status = TaskStatus::Interrupted;
            record.info.error = Some("Core exited before this task completed".into());
            self.event(
                &mut record,
                "task.interrupted",
                serde_json::json!({"reason":"core_restart"}),
            )
            .await?;
        }
        Ok(record.info)
    }

    pub async fn cancel(&self, id: &str) -> Result<(), String> {
        let info = self.get(id).await?;
        if !matches!(info.status, TaskStatus::Queued | TaskStatus::Running) {
            return Err("Task is already complete".into());
        }
        let sender = self
            .active
            .lock()
            .map_err(|_| "Task manager lock poisoned".to_string())?
            .get(id)
            .cloned()
            .ok_or("Task is not active in this Core process")?;
        sender
            .send(true)
            .map_err(|_| "Task cancellation channel is closed".to_string())
    }

    pub async fn events(
        &self,
        id: &str,
        after: u64,
        wait_ms: u64,
    ) -> Result<Vec<TaskEvent>, String> {
        validate_task_id(id)?;
        let wait = Duration::from_millis(wait_ms.min(30_000));
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            let path = self.root.join("tasks/events").join(format!("{id}.jsonl"));
            super::engine::ensure_no_symlink(&self.root, &path).await?;
            if let Ok(raw) = tokio::fs::read_to_string(&path).await {
                let events = raw
                    .lines()
                    .filter_map(|line| serde_json::from_str::<TaskEvent>(line).ok())
                    .filter(|event| event.sequence > after)
                    .collect::<Vec<_>>();
                if !events.is_empty() {
                    return Ok(events);
                }
            }
            let info = self.get(id).await?;
            if !matches!(info.status, TaskStatus::Queued | TaskStatus::Running)
                || tokio::time::Instant::now() >= deadline
            {
                return Ok(Vec::new());
            }
            tokio::time::sleep(
                Duration::from_millis(100)
                    .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
            )
            .await;
        }
    }

    pub async fn recover(&self) -> Result<(), String> {
        let dir = self.root.join("tasks");
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| e.to_string())?;
        while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
            if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            super::engine::ensure_no_symlink(&self.root, &entry.path()).await?;
            let bytes = tokio::fs::read(entry.path())
                .await
                .map_err(|e| e.to_string())?;
            let mut record: TaskRecord =
                serde_json::from_slice(&bytes).map_err(|e| format!("Corrupt task record: {e}"))?;
            if matches!(record.info.status, TaskStatus::Queued | TaskStatus::Running) {
                record.info.status = TaskStatus::Interrupted;
                record.info.error = Some("Core exited before this task completed".into());
                self.event(
                    &mut record,
                    "task.interrupted",
                    serde_json::json!({"reason":"core_restart"}),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn read(&self, id: &str) -> Result<TaskRecord, String> {
        validate_task_id(id)?;
        let path = self.root.join("tasks").join(format!("{id}.json"));
        super::engine::ensure_no_symlink(&self.root, &path).await?;
        let bytes = tokio::fs::read(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Task not found".into()
            } else {
                e.to_string()
            }
        })?;
        serde_json::from_slice(&bytes).map_err(|e| format!("Corrupt task record: {e}"))
    }
}

impl TaskContext {
    pub fn emit(&self, event: &str, data: Value) {
        let _ = self.updates.send((event.to_owned(), data));
    }
    pub fn cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }
    pub async fn wait_cancelled(&self) {
        let mut receiver = self.cancelled.clone();
        if !*receiver.borrow() {
            let _ = receiver.changed().await;
        }
    }
    pub async fn download_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        let mut receiver = self.cancelled.clone();
        if *receiver.borrow() {
            return Err("Task cancelled".into());
        }
        tokio::select! {
            permit = self.downloads.clone().acquire_owned() => permit.map_err(|e| e.to_string()),
            _ = receiver.changed() => Err("Task cancelled".into()),
        }
    }
}

fn validate_task_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Invalid task ID".into());
    }
    Ok(())
}

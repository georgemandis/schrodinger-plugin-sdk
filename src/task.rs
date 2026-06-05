// src/task.rs
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::types::Output;

/// Result of a completed task.
pub enum TaskResult {
    Outputs(Vec<Output>),
    AnalysisDone,
    Error(String),
}

/// Status returned by poll.
pub enum TaskStatus {
    Pending,
    Complete,
    Failed,
}

struct TaskState {
    status: TaskStatus,
    progress: f32,
    result: Option<TaskResult>,
}

/// Manages background task handles for the poll-based async model.
pub struct TaskManager {
    next_id: AtomicU64,
    tasks: Mutex<HashMap<u64, Arc<Mutex<TaskState>>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager {
            next_id: AtomicU64::new(1),
            tasks: Mutex::new(HashMap::new()),
        }
    }

    /// Spawn a task on a background thread. Returns handle for polling.
    pub fn spawn<F>(&self, f: F) -> u64
    where
        F: FnOnce() -> TaskResult + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let state = Arc::new(Mutex::new(TaskState {
            status: TaskStatus::Pending,
            progress: 0.0,
            result: None,
        }));

        let state_clone = Arc::clone(&state);
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let mut s = state_clone.lock().unwrap();
            match result {
                Ok(task_result) => {
                    match &task_result {
                        TaskResult::Error(_) => s.status = TaskStatus::Failed,
                        _ => s.status = TaskStatus::Complete,
                    }
                    s.progress = 1.0;
                    s.result = Some(task_result);
                }
                Err(panic_info) => {
                    let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "plugin panicked".to_string()
                    };
                    s.status = TaskStatus::Failed;
                    s.progress = 1.0;
                    s.result = Some(TaskResult::Error(msg));
                }
            }
        });

        self.tasks.lock().unwrap().insert(id, state);
        id
    }

    /// Poll task status. Returns (status_code, progress).
    /// status_code: 0=pending, 1=complete, -1=error
    pub fn poll(&self, handle: u64) -> (i32, f32) {
        let tasks = self.tasks.lock().unwrap();
        match tasks.get(&handle) {
            Some(state) => {
                let s = state.lock().unwrap();
                let code = match s.status {
                    TaskStatus::Pending => 0,
                    TaskStatus::Complete => 1,
                    TaskStatus::Failed => -1,
                };
                (code, s.progress)
            }
            None => (-1, 0.0),
        }
    }

    /// Take the result for a completed task.
    pub fn take_result(&self, handle: u64) -> Option<TaskResult> {
        let tasks = self.tasks.lock().unwrap();
        if let Some(state) = tasks.get(&handle) {
            let mut s = state.lock().unwrap();
            s.result.take()
        } else {
            None
        }
    }

    /// Remove a task and free its resources.
    pub fn remove(&self, handle: u64) {
        self.tasks.lock().unwrap().remove(&handle);
    }
}

use std::sync::Mutex;

use chrono::Utc;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize)]
pub struct Process {
    pub id: u64,
    pub name: String,
    pub category: String,
    pub status: ProcessStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub progress: Option<f64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProcessStatus {
    Running,
    Completed,
    Failed,
    Pending,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static PROCESSES: Lazy<Mutex<Vec<Process>>> = Lazy::new(|| Mutex::new(Vec::new()));
const MAX_PROCESSES: usize = 500;

pub fn register(name: &str, category: &str, message: Option<&str>) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let process = Process {
        id,
        name: name.to_string(),
        category: category.to_string(),
        status: ProcessStatus::Running,
        started_at: Some(Utc::now().to_rfc3339()),
        finished_at: None,
        progress: None,
        message: message.map(|s| s.to_string()),
    };
    let mut list = PROCESSES.lock().unwrap();
    list.push(process);
    trim_if_needed(&mut list);
    id
}

pub fn update(id: u64, progress: Option<f64>, message: Option<&str>) {
    if let Some(p) = PROCESSES.lock().unwrap().iter_mut().find(|p| p.id == id) {
        p.progress = progress;
        if let Some(msg) = message {
            p.message = Some(msg.to_string());
        }
    }
}

pub fn complete(id: u64, message: Option<&str>) {
    if let Some(p) = PROCESSES.lock().unwrap().iter_mut().find(|p| p.id == id) {
        p.status = ProcessStatus::Completed;
        p.finished_at = Some(Utc::now().to_rfc3339());
        p.progress = Some(100.0);
        if let Some(msg) = message {
            p.message = Some(msg.to_string());
        }
    }
}

pub fn fail(id: u64, message: &str) {
    if let Some(p) = PROCESSES.lock().unwrap().iter_mut().find(|p| p.id == id) {
        p.status = ProcessStatus::Failed;
        p.finished_at = Some(Utc::now().to_rfc3339());
        p.message = Some(message.to_string());
    }
}

pub fn pending(name: &str, category: &str, message: Option<&str>) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let process = Process {
        id,
        name: name.to_string(),
        category: category.to_string(),
        status: ProcessStatus::Pending,
        started_at: None,
        finished_at: None,
        progress: None,
        message: message.map(|s| s.to_string()),
    };
    let mut list = PROCESSES.lock().unwrap();
    list.push(process);
    trim_if_needed(&mut list);
    id
}

pub fn get_all() -> Vec<Process> {
    PROCESSES.lock().unwrap().clone()
}

pub fn clear_completed() {
    PROCESSES
        .lock()
        .unwrap()
        .retain(|p| p.status == ProcessStatus::Running || p.status == ProcessStatus::Pending);
}

fn trim_if_needed(list: &mut Vec<Process>) {
    while list.len() > MAX_PROCESSES {
        if let Some(idx) = list.iter().rposition(|p| {
            p.status == ProcessStatus::Completed || p.status == ProcessStatus::Failed
        }) {
            list.remove(idx);
        } else {
            break;
        }
    }
}

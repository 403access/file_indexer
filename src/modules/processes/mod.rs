use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use once_cell::sync::Lazy;
use serde::Serialize;

use crate::modules::logging;

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
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProcessStatus {
    Running,
    Completed,
    Failed,
    Pending,
}

#[derive(Debug, Clone)]
pub struct ProcessControl {
    pub stop: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static PROCESSES: Lazy<Mutex<Vec<Process>>> = Lazy::new(|| Mutex::new(Vec::new()));
static CONTROLS: Lazy<Mutex<std::collections::HashMap<u64, ProcessControl>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));
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
        paused: false,
    };
    let mut list = PROCESSES.lock().unwrap();
    list.push(process);
    trim_if_needed(&mut list);
    logging::info_with_process(&format!("Process registered: {} ({})", name, category), id);
    id
}

pub fn register_controllable(name: &str, category: &str, message: Option<&str>) -> u64 {
    let id = register(name, category, message);
    let mut controls = CONTROLS.lock().unwrap();
    controls.insert(id, ProcessControl {
        stop: Arc::new(AtomicBool::new(false)),
        pause: Arc::new(AtomicBool::new(false)),
    });
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
    logging::info_with_process(&format!("Process completed: {}", message.unwrap_or("done")), id);
    remove_control(id);
}

pub fn fail(id: u64, message: &str) {
    if let Some(p) = PROCESSES.lock().unwrap().iter_mut().find(|p| p.id == id) {
        p.status = ProcessStatus::Failed;
        p.finished_at = Some(Utc::now().to_rfc3339());
        p.message = Some(message.to_string());
    }
    logging::error_with_process(&format!("Process failed: {}", message), id);
    remove_control(id);
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
        paused: false,
    };
    let mut list = PROCESSES.lock().unwrap();
    list.push(process);
    trim_if_needed(&mut list);
    id
}

pub fn get_all() -> Vec<Process> {
    PROCESSES.lock().unwrap().clone()
}

pub fn get_control(id: u64) -> Option<ProcessControl> {
    CONTROLS.lock().unwrap().get(&id).cloned()
}

pub fn is_stopped(id: u64) -> bool {
    CONTROLS
        .lock()
        .unwrap()
        .get(&id)
        .map(|c| c.stop.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub fn is_paused(id: u64) -> bool {
    CONTROLS
        .lock()
        .unwrap()
        .get(&id)
        .map(|c| c.pause.load(Ordering::SeqCst))
        .unwrap_or(false)
}

pub fn set_paused(id: u64, paused: bool) {
    if let Some(c) = CONTROLS.lock().unwrap().get(&id) {
        c.pause.store(paused, Ordering::SeqCst);
        if let Some(p) = PROCESSES.lock().unwrap().iter_mut().find(|p| p.id == id) {
            p.paused = paused;
        }
    }
}

pub fn request_stop(id: u64) {
    if let Some(c) = CONTROLS.lock().unwrap().get(&id) {
        c.stop.store(true, Ordering::SeqCst);
    }
}

pub fn clear_completed() {
    PROCESSES
        .lock()
        .unwrap()
        .retain(|p| p.status == ProcessStatus::Running || p.status == ProcessStatus::Pending);
    CONTROLS.lock().unwrap().retain(|id, c| {
        let keep = PROCESSES.lock().unwrap().iter().any(|p| p.id == *id && (p.status == ProcessStatus::Running || p.status == ProcessStatus::Pending));
        if !keep {
            c.stop.store(true, Ordering::SeqCst);
        }
        keep
    });
}

fn remove_control(id: u64) {
    if let Some(c) = CONTROLS.lock().unwrap().remove(&id) {
        c.stop.store(true, Ordering::SeqCst);
    }
}

fn trim_if_needed(list: &mut Vec<Process>) {
    while list.len() > MAX_PROCESSES {
        if let Some(idx) = list.iter().rposition(|p| {
            p.status == ProcessStatus::Completed || p.status == ProcessStatus::Failed
        }) {
            let pid = list[idx].id;
            list.remove(idx);
            remove_control(pid);
        } else {
            break;
        }
    }
}

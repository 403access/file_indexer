use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub status: String,
    pub total_entries: usize,
    pub current_dir: String,
    pub started_at: Option<String>,
    pub elapsed_secs: Option<f64>,
}

static PROGRESS: Lazy<Mutex<Progress>> = Lazy::new(|| {
    Mutex::new(Progress {
        status: "idle".to_string(),
        total_entries: 0,
        current_dir: String::new(),
        started_at: None,
        elapsed_secs: None,
    })
});

pub fn start(total_entries: usize) {
    let mut p = PROGRESS.lock().unwrap();
    p.status = "indexing".to_string();
    p.total_entries = total_entries;
    p.current_dir = String::new();
    p.started_at = Some(chrono::Utc::now().to_rfc3339());
    p.elapsed_secs = None;
}

pub fn update_dir(dir: &str, total_entries: usize) {
    let mut p = PROGRESS.lock().unwrap();
    p.current_dir = dir.to_string();
    p.total_entries = total_entries;
    if let Some(ref started) = p.started_at {
        if let Ok(start) = chrono::DateTime::parse_from_rfc3339(started) {
            p.elapsed_secs = Some((chrono::Utc::now() - start.with_timezone(&chrono::Utc)).num_milliseconds() as f64 / 1000.0);
        }
    }
}

pub fn finish() {
    let mut p = PROGRESS.lock().unwrap();
    p.status = "idle".to_string();
    p.current_dir = String::new();
    if let Some(ref started) = p.started_at {
        if let Ok(start) = chrono::DateTime::parse_from_rfc3339(started) {
            p.elapsed_secs = Some((chrono::Utc::now() - start.with_timezone(&chrono::Utc)).num_milliseconds() as f64 / 1000.0);
        }
    }
}

pub fn get_progress() -> Progress {
    PROGRESS.lock().unwrap().clone()
}

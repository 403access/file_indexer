use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Application state - holds runtime configuration and state
#[derive(Clone, Debug)]
pub struct AppState {
    pub cwd: String,
    pub db: String,
    pub pause_indexer: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cwd: String::new(),
            db: String::new(),
            pause_indexer: Arc::new(AtomicBool::new(false)),
        }
    }
}

// Thread-local storage for app state
thread_local! {
    static APP_STATE: RefCell<AppState> = RefCell::new(AppState::default());
}

/// Initialize the app state with the given working directory
pub fn init(cwd: String, db: String) {
    APP_STATE.with(|state| {
        *state.borrow_mut() = AppState {
            cwd,
            db,
            pause_indexer: Arc::new(AtomicBool::new(false)),
        };
    });
}

/// Get the current working directory
pub fn get_cwd() -> String {
    APP_STATE.with(|state| state.borrow().cwd.clone())
}

/// Set the current working directory
pub fn set_cwd(cwd: String) {
    APP_STATE.with(|state| {
        state.borrow_mut().cwd = cwd;
    });
}

/// Get the database connection string
pub fn get_db() -> String {
    APP_STATE.with(|state| state.borrow().db.clone())
}

/// Set the database connection string
pub fn set_db(db: String) {
    APP_STATE.with(|state| {
        state.borrow_mut().db = db;
    });
}

/// Pause the indexer (call before handling web requests)
pub fn pause_indexer(state: &AppState) {
    state.pause_indexer.store(true, Ordering::SeqCst);
}

/// Resume the indexer (call after web request completes)
pub fn resume_indexer(state: &AppState) {
    state.pause_indexer.store(false, Ordering::SeqCst);
}

/// Check if indexer should pause
pub fn should_pause(state: &AppState) -> bool {
    state.pause_indexer.load(Ordering::SeqCst)
}

/// Guard that pauses indexer on creation and resumes on drop
pub struct IndexerPauseGuard {
    state: AppState,
}

impl IndexerPauseGuard {
    pub fn new(state: &AppState) -> Self {
        pause_indexer(state);
        Self { state: state.clone() }
    }
}

impl Drop for IndexerPauseGuard {
    fn drop(&mut self) {
        resume_indexer(&self.state);
    }
}

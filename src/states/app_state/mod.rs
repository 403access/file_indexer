use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Application state - holds runtime configuration and state
#[derive(Clone, Debug)]
pub struct AppState {
    pub cwd: String,
    pub db: String,
    /// Number of active web requests that need the indexer to yield.
    /// Use refcount so concurrent requests don't resume early.
    pub pause_indexer: Arc<AtomicUsize>,
    /// Whether startup indexing is enabled (ENABLE_STARTUP_INDEXING).
    pub enable_startup_indexing: bool,
    /// Whether the initial dashboard refresh is enabled (ENABLE_INITIAL_DASHBOARD_REFRESH).
    pub enable_initial_dashboard_refresh: bool,
    /// Whether periodic dashboard refresh is enabled (ENABLE_DASHBOARD_REFRESH).
    pub enable_dashboard_refresh: bool,
    /// Whether duplicate folder groups refresh is enabled (ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH).
    pub enable_duplicate_folder_groups_refresh: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cwd: String::new(),
            db: String::new(),
            pause_indexer: Arc::new(AtomicUsize::new(0)),
            enable_startup_indexing: true,
            enable_initial_dashboard_refresh: true,
            enable_dashboard_refresh: true,
            enable_duplicate_folder_groups_refresh: true,
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
            pause_indexer: Arc::new(AtomicUsize::new(0)),
            enable_startup_indexing: true,
            enable_initial_dashboard_refresh: true,
            enable_dashboard_refresh: true,
            enable_duplicate_folder_groups_refresh: true,
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
    state.pause_indexer.fetch_add(1, Ordering::SeqCst);
}

/// Resume the indexer (call after web request completes)
pub fn resume_indexer(state: &AppState) {
    // Saturating sub so a bug elsewhere can't wrap
    let mut current = state.pause_indexer.load(Ordering::SeqCst);
    while current > 0 {
        match state.pause_indexer.compare_exchange_weak(
            current,
            current - 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => break,
            Err(v) => current = v,
        }
    }
}

/// Check if indexer should pause
pub fn should_pause(state: &AppState) -> bool {
    state.pause_indexer.load(Ordering::SeqCst) > 0
}

/// Guard that pauses indexer on creation and resumes on drop
pub struct IndexerPauseGuard {
    state: AppState,
}

impl IndexerPauseGuard {
    pub fn new(state: &AppState) -> Self {
        pause_indexer(state);
        Self {
            state: state.clone(),
        }
    }
}

impl Drop for IndexerPauseGuard {
    fn drop(&mut self) {
        resume_indexer(&self.state);
    }
}

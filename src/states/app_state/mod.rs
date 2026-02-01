use std::cell::RefCell;

/// Application state - holds runtime configuration and state
#[derive(Clone, Debug)]
pub struct AppState {
    pub cwd: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self { cwd: String::new() }
    }
}

// Thread-local storage for app state
thread_local! {
    static APP_STATE: RefCell<AppState> = RefCell::new(AppState::default());
}

/// Initialize the app state with the given working directory
pub fn init(cwd: String) {
    APP_STATE.with(|state| {
        *state.borrow_mut() = AppState { cwd };
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

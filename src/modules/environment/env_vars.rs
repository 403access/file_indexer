use std::cell::RefCell;
use std::env;
use std::path::Path;

/// Structure to hold environment variables
#[derive(Clone, Debug)]
pub struct EnvironmentVariables {
    pub database_url: String,
    pub cwd: String,
    pub enable_startup_indexing: bool,
    pub enable_dashboard_refresh: bool,
    pub enable_duplicate_folder_groups_refresh: bool,
    pub duplicate_folder_groups_refresh_interval: u64,
}

impl Default for EnvironmentVariables {
    fn default() -> Self {
        Self {
            database_url: String::from("file_index.db"),
            cwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            enable_startup_indexing: true,
            enable_dashboard_refresh: true,
            enable_duplicate_folder_groups_refresh: true,
            duplicate_folder_groups_refresh_interval: 120,
        }
    }
}

fn derive_db_name(cwd: &str) -> String {
    let folder_name = Path::new(cwd)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file_index");
    format!("{}.db", folder_name)
}

// Thread-local storage for environment variables
thread_local! {
    static ENV_VARS: RefCell<EnvironmentVariables> = RefCell::new(EnvironmentVariables::default());
}

/// Load environment variables from .env file and environment
pub fn load() {
    dotenvy::dotenv().ok(); // Load .env file

    ENV_VARS.with(|vars| {
        let mut env_vars = vars.borrow_mut();

        env_vars.cwd = env::var("CWD").unwrap_or_else(|_| EnvironmentVariables::default().cwd);

        env_vars.database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| derive_db_name(&env_vars.cwd));

        env_vars.enable_startup_indexing = env::var("ENABLE_STARTUP_INDEXING")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);

        env_vars.enable_dashboard_refresh = env::var("ENABLE_DASHBOARD_REFRESH")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);

        env_vars.enable_duplicate_folder_groups_refresh = env::var("ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);

        env_vars.duplicate_folder_groups_refresh_interval = env::var("DUPLICATE_FOLDER_GROUPS_REFRESH_INTERVAL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120);

        env_vars.enable_duplicate_folder_groups_refresh = env::var("ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true);

        env_vars.duplicate_folder_groups_refresh_interval = env::var("DUPLICATE_FOLDER_GROUPS_REFRESH_INTERVAL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120);
    });
}

/// Get the current database URL
pub fn get_database_url() -> String {
    ENV_VARS.with(|vars| vars.borrow().database_url.clone())
}

/// Get the current working directory
pub fn get_cwd() -> String {
    ENV_VARS.with(|vars| vars.borrow().cwd.clone())
}

/// Get the server port to bind the HTTP API to (default: 3000)
///
/// Reads the `PORT` environment variable. Falls back to `3000` when the
/// variable is unset or not a valid port number.
pub fn get_server_port() -> u16 {
    env::var("PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(3000)
}

/// Whether startup indexing is enabled (default: true)
///
/// Reads the `ENABLE_STARTUP_INDEXING` environment variable. When set to
/// `false`, the application will skip the automatic indexing pass at startup.
pub fn get_enable_startup_indexing() -> bool {
    ENV_VARS.with(|vars| vars.borrow().enable_startup_indexing)
}

/// Whether periodic dashboard refresh is enabled (default: true)
///
/// Reads the `ENABLE_DASHBOARD_REFRESH` environment variable. When set to
/// `false`, the background task that recomputes dashboard stats will not run.
pub fn get_enable_dashboard_refresh() -> bool {
    ENV_VARS.with(|vars| vars.borrow().enable_dashboard_refresh)
}

/// Whether duplicate folder groups materialization refresh is enabled (default: true)
///
/// Reads the `ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH` environment variable.
pub fn get_enable_duplicate_folder_groups_refresh() -> bool {
    ENV_VARS.with(|vars| vars.borrow().enable_duplicate_folder_groups_refresh)
}

/// Interval in seconds for duplicate folder groups materialization refresh (default: 120)
///
/// Reads the `DUPLICATE_FOLDER_GROUPS_REFRESH_INTERVAL` environment variable.
pub fn get_duplicate_folder_groups_refresh_interval() -> u64 {
    ENV_VARS.with(|vars| vars.borrow().duplicate_folder_groups_refresh_interval)
}

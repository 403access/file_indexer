use std::cell::RefCell;
use std::env;
use std::path::Path;

/// Structure to hold environment variables
#[derive(Clone, Debug)]
pub struct EnvironmentVariables {
    pub database_url: String,
    pub cwd: String,
}

impl Default for EnvironmentVariables {
    fn default() -> Self {
        Self {
            database_url: String::from("file_index.db"),
            cwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
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

use std::cell::RefCell;
use std::env;

/// Structure to hold environment variables
#[derive(Clone, Debug)]
pub struct EnvironmentVariables {
    pub database_url: String,
    pub cwd: String,
}

impl Default for EnvironmentVariables {
    fn default() -> Self {
        Self {
            database_url: String::from("file_index.db"), // Default to file_index.db
            cwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
        }
    }
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

        env_vars.database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| EnvironmentVariables::default().database_url);

        env_vars.cwd = env::var("CWD").unwrap_or_else(|_| EnvironmentVariables::default().cwd);
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

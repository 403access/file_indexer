pub fn check_vars() {
    super::env_vars::load();

    println!("✅ Environment Variables Loaded:");
    println!("DATABASE_URL: {}", super::env_vars::get_database_url());
    println!("CWD: {}", super::env_vars::get_cwd());
    println!("ENABLE_STARTUP_INDEXING: {}", super::env_vars::get_enable_startup_indexing());
    println!("ENABLE_INITIAL_DASHBOARD_REFRESH: {}", super::env_vars::get_enable_initial_dashboard_refresh());
    println!("ENABLE_DASHBOARD_REFRESH: {}", super::env_vars::get_enable_dashboard_refresh());
    println!("ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH: {}", super::env_vars::get_enable_duplicate_folder_groups_refresh());
    println!("DUPLICATE_FOLDER_GROUPS_REFRESH_INTERVAL: {}", super::env_vars::get_duplicate_folder_groups_refresh_interval());
    println!("IGNORE_PROCESS_DATABASE_STATE: {}", super::env_vars::get_ignore_process_database_state());
}

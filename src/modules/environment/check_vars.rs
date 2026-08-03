pub fn check_vars() {
    super::env_vars::load();

    println!("✅ Environment Variables Loaded:");
    println!("DATABASE_URL: {}", super::env_vars::get_database_url());
    println!("CWD: {}", super::env_vars::get_cwd());
    println!("ENABLE_STARTUP_INDEXING: {}", super::env_vars::get_enable_startup_indexing());
    println!("ENABLE_DASHBOARD_REFRESH: {}", super::env_vars::get_enable_dashboard_refresh());
}

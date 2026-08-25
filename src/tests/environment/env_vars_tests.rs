use crate::modules::environment::env_vars::{
    get_enable_dashboard_refresh, get_enable_duplicate_folder_groups_refresh,
    get_enable_initial_dashboard_refresh, get_enable_startup_indexing,
    get_duplicate_folder_groups_refresh_interval,
};

#[test]
fn default_env_vars_are_sensible() {
    assert!(get_enable_startup_indexing());
    assert!(get_enable_initial_dashboard_refresh());
    assert!(get_enable_dashboard_refresh());
    assert!(get_enable_duplicate_folder_groups_refresh());
    assert_eq!(get_duplicate_folder_groups_refresh_interval(), 120);
}

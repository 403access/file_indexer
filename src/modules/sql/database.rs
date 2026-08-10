// Re-export facade for backward compatibility.
// Prefer importing directly from the specific submodules.

pub use super::connection::{get_connection, run_blocking};
pub use super::dashboard::{recompute_dashboard_stats, refresh_dashboard_stats};
pub use super::duplicate_folders_materialized::{
    create_duplicate_folder_groups_table, refresh_duplicate_folder_groups,
};
pub use super::duplicates::{
    create_duplicates_table, refresh_duplicate_hashes, remove_duplicates_table,
    reset_duplicates_table, update_duplicate_hashes_incremental,
};
pub use super::files::{
    delete_directory_tree, delete_stale_children, get_child_directories, get_child_entries,
    get_directory_metadata, get_or_insert_file_name, insert_file, insert_file_name,
    is_directory_indexed, mark_directory_traversed, mark_directory_traversed_and_modified,
    upsert_file, DirectoryMetadata, UpsertResult,
};
pub use super::logs::{count_ignore_events, count_logs, get_logs, insert_log};
pub use super::schema::init_db;
pub use super::settings::{
    get_ignore_list, get_ignore_rules, get_setting, set_ignore_list, set_ignore_rules, set_setting,
    IgnoreRule,
};
pub use super::skipped::{
    count_skipped_paths, count_skipped_paths_filtered, get_skipped_paths, get_skipped_paths_page,
    get_skipped_paths_page_filtered, insert_skipped_path, SkippedMatchField, SkippedSortField,
};

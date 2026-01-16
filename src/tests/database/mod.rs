use std::fs;

use crate::modules::commands::{
    command_init_db::command_init_db, commands_progress_bar::create_progress_bar,
};

#[test]
pub fn create_database() {
    let pb = create_progress_bar();

    let result = command_init_db(&pb);
    assert!(result.is_ok());

    // Check file existence
    assert!(fs::metadata("file_index.db").is_err());
}

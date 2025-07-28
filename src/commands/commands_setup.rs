use indicatif::ProgressBar;
use std::io;

use crate::commands::command_exit::command_exit;
use crate::commands::command_index_duplicates::command_index_duplicates;
use crate::commands::command_index_files::command_index_files;
use crate::commands::command_init_db::command_init_db;
use crate::commands::command_list_duplicates::command_list_duplicates;
use crate::commands::command_search_file::command_search_file;

pub struct Command<'a> {
    pub name: &'a str,
    pub action: fn(&ProgressBar) -> io::Result<bool>,
}

impl<'a> Command<'a> {
    pub const fn new(name: &'a str, action: fn(&ProgressBar) -> io::Result<bool>) -> Self {
        Command { name, action }
    }
}

pub const COMMAND_EXIT: Command<'static> = Command::new("Exit", command_exit);

pub const COMMAND_INIT_DB: Command<'_> = Command::new("Init database", command_init_db);

pub const COMMAND_INDEX_FILES: Command<'_> = Command::new("Index files", command_index_files);

pub const COMMAND_SEARCH_FILE: Command<'_> = Command::new("Search file", command_search_file);

pub const COMMAND_INDEX_DUPLICATES: Command<'_> =
    Command::new("Index duplicates", command_index_duplicates);

pub const COMMAND_LIST_DUPLICATES: Command<'_> =
    Command::new("List duplicates", command_list_duplicates);

pub fn build_commands() -> Vec<Command<'static>> {
    vec![
        COMMAND_INIT_DB,
        COMMAND_INDEX_FILES,
        COMMAND_SEARCH_FILE,
        COMMAND_INDEX_DUPLICATES,
        COMMAND_LIST_DUPLICATES,
    ]
}

pub fn validate_commands(commands: &Vec<Command<'static>>) -> io::Result<()> {
    if commands.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No commands provided. At least one command is required.",
        ));
    }

    let mut seen = std::collections::HashSet::new();
    for cmd in commands {
        if !seen.insert(cmd.name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Duplicate command name found: {}", cmd.name),
            ));
        }
    }

    Ok(())
}

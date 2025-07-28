pub mod file_entry {
    pub mod _types;
    pub mod convert;
    pub mod sort;
}
pub mod index_files {
    pub mod _command;
    pub mod _types;
}
pub mod search_files {
    pub mod try_get_dir_entries;
}
pub mod sql {
    pub mod database;
    // pub mod insert_file;
    pub mod search;
    pub mod duplicates;
}

pub mod services {
    pub mod duplicate_service;
    pub mod index_service;
    pub mod search_service;
}

pub mod commands {
    pub mod commands_loop;
    pub mod commands_progress_bar;
    pub mod commands_setup;

    pub mod command_exit;
    pub mod command_index_files;
    pub mod command_init_db;
    pub mod command_search_file;

    // Duplicates
    pub mod command_index_duplicates;
    pub mod command_list_duplicates;
}

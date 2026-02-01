use std::{env, io};

use file_indexer::modules::{
    arguments::check_arguments::check_arguments, commands::commands_loop::commands_loop,
    environment::check_vars::check_vars,
};
use file_indexer::states::app_state;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    check_arguments(&args).unwrap();

    // Load environment variables
    check_vars();
    let path = file_indexer::modules::environment::env_vars::get_cwd();
    let database_url = file_indexer::modules::environment::env_vars::get_database_url();

    // Initialize application state
    app_state::init(path, database_url);

    commands_loop()
}

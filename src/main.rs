use std::{env, io};

use file_indexer::modules::{
    arguments::check_arguments::check_arguments, commands::commands_loop::commands_loop,
};

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let path = check_arguments(&args).unwrap();
    println!("Starting in directory: {}", path);

    commands_loop()
}

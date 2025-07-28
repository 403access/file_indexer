use std::io;

use file_indexer::commands::commands_loop::commands_loop;

fn main() -> io::Result<()> {
    commands_loop()
}

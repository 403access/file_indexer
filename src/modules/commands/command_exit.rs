use std::io;

use indicatif::ProgressBar;

pub fn command_exit(pb: &ProgressBar) -> io::Result<bool> {
    println!("Exiting...");
    Ok(true)
}

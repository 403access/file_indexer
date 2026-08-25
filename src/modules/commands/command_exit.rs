use std::io;

use indicatif::ProgressBar;

pub fn command_exit(_pb: &ProgressBar) -> io::Result<bool> {
    println!("Exiting...");
    Ok(true)
}

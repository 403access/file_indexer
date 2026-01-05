use std::io;

use indicatif::ProgressBar;

pub fn command_index_duplicates(pb: &ProgressBar) -> io::Result<bool> {
    // Simulate indexing duplicates
    pb.println("[info] Starting to index duplicates...");

    // Simulate some work
    for i in 1..=3 {
        std::thread::sleep(std::time::Duration::from_millis(800));
        pb.println(format!("[info] Step {} done for indexing duplicates", i));
    }

    pb.println("[info] Finished indexing duplicates.");

    Ok(false)
}

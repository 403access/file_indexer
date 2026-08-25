use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/** Spinner while executing */
pub fn create_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new_spinner();

    pb.enable_steady_tick(Duration::from_millis(100));

    let style = ProgressStyle::default_spinner()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
        .template("{spinner} {msg}")
        .expect("Failed to set progress bar template");
    pb.set_style(style);

    return pb;
}

pub fn set_runnning_message(pb: &ProgressBar, message: &str) {
    pb.set_message(format!("Running: {}", message));
}

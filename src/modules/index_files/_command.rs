use std::{fs, io};

/**
 * Validate the path
 */
pub fn check_input(path: &str) -> Result<(), io::Error> {
    let trimmed_path = path.trim();
    if trimmed_path.trim().is_empty() {
        eprintln!("trimmedPath is empty");
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trimmedPath cannot be empty",
        ));
    }
    if !trimmed_path.starts_with('/') {
        eprintln!(
            "trimmedPath must be an absolute trimmedPath: {}",
            trimmed_path
        );
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trimmedPath must be absolute",
        ));
    }
    if !trimmed_path.ends_with('/') {
        eprintln!("trimmedPath must end with a slash: {}", trimmed_path);
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trimmedPath must end with a slash",
        ));
    }
    if !fs::metadata(trimmed_path).is_ok() {
        eprintln!(
            "trimmedPath does not exist or is not accessible: {}",
            trimmed_path
        );
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "trimmedPath not found",
        ));
    }

    Ok(())
}

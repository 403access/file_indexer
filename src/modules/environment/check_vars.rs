use std::env;
use std::error::Error;

pub fn check_vars() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file.
    // Fails if .env file not found, not readable or invalid.
    dotenvy::dotenv()?;

    // Print all environment variables for debugging purposes.
    println!("Environment variables:");
    for (key, value) in env::vars() {
        println!("{key}: {value}");
    }

    Ok(())
}

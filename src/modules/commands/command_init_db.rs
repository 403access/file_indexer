use std::io::{self, Error};
use std::path::{Path, PathBuf};

use clap::builder::Str;
use indicatif::ProgressBar;

use crate::modules::sql::database::{get_connection, init_db};

pub fn _init_db(database_file_path: &PathBuf) -> io::Result<(Str, Error)> {
    let mut conn = get_connection(database_file_path.to_str().unwrap()).map_err(|e| {
        eprintln!("Failed to connect to database: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;
    println!("Database connection established.");

    // get transaction or return error
    let transaction = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("Failed to start transaction: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    };

    // let _init_db_result = match init_db(&transaction) {
    //     Ok(()) => {
    //         println!("Database initialized.");
    //         Ok(())
    //     }
    //     Err(e) => {
    //         eprintln!("Database initialization error: {}", e);
    //         Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
    //     }
    // };

    let init_db_result = init_db(&transaction);
    if init_db_result.is_err() {
        eprintln!("Database initialization failed.");
        return Err(io::Error::new(
            io::ErrorKind::Other,
            init_db_result.unwrap_err().to_string(),
        ));
    }
    println!("Database initialized successfully.");
    transaction.commit().map_err(|e| {
        eprintln!("Failed to commit transaction: {}", e);
        io::Error::new(io::ErrorKind::Other, e.to_string())
    })?;
    println!("Transaction committed successfully.");

    // Ok(false)
    return Err(io::Error::new(
        io::ErrorKind::Other,
        "Unknown state of initialization hit.",
    ));
}

pub fn command_init_db(_pb: &ProgressBar) -> io::Result<bool> {
    let database_file_path_buf = Path::new("file_index.db").to_path_buf();
    let result = _init_db(&database_file_path_buf);
    if result.is_err() {
        return Ok(false);
    }
    return Ok(true);
}

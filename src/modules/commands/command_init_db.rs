use std::io;

use indicatif::ProgressBar;

use crate::modules::sql::database::{get_connection, init_db};

pub fn command_init_db(_pb: &ProgressBar) -> io::Result<bool> {
    let mut conn = get_connection("file_index.db").map_err(|e| {
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

    Ok(false)
}

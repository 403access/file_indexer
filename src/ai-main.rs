use rusqlite::{params, Connection, Result};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::PathBuf, time::SystemTime};
use walkdir::WalkDir;
use clap::{Parser, Subcommand};

#[derive(Debug)]
struct FileEntry {
    path: String,
    size: u64,
    modified: SystemTime,
    hash: Option<String>,
}

#[derive(Parser)]
#[command(name = "FileIndexer")]
#[command(about = "Index and search files/directories")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a directory
    Index {
        /// Path to directory
        #[arg(short, long)]
        dir: String,
    },
    /// Search for files or directories by name
    Search {
        /// Substring to match (case-insensitive)
        #[arg(short, long)]
        name: String,
    },
}

fn hash_file(path: &PathBuf) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 4096];

    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Some(format!("{:x}", hasher.finalize()))
}

fn setup_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE,
            size INTEGER NOT NULL,
            modified INTEGER NOT NULL,
            hash TEXT
        );

        CREATE TABLE IF NOT EXISTS directories (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE
        );
        ",
    )?;
    Ok(())
}

fn insert_directory(conn: &Connection, path: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO directories (path) VALUES (?1)",
        params![path],
    )?;
    Ok(())
}

fn insert_file(conn: &Connection, file: &FileEntry) -> Result<()> {
    let modified = file
        .modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO files (path, size, modified, hash) VALUES (?1, ?2, ?3, ?4)",
        params![file.path, file.size, modified, file.hash],
    )?;
    Ok(())
}

fn index_directory(root: &str, conn: &Connection) -> Result<()> {
    let root_path = PathBuf::from(root);
    for entry in WalkDir::new(&root_path).into_iter().flatten() {
        let path = entry.path();
        let path_str = path.to_string_lossy().to_string();

        if path.is_dir() {
            insert_directory(conn, &path_str)?;
        } else if path.is_file() {
            let metadata = fs::metadata(path)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let hash = hash_file(&path.to_path_buf());
            let file = FileEntry {
                path: path_str,
                size: metadata.len(),
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                hash,
            };
            insert_file(conn, &file)?;
        }
    }

    Ok(())
}

fn search(conn: &Connection, keyword: &str) -> Result<()> {
    let keyword = format!("%{}%", keyword.to_lowercase());

    println!("🔍 Matching directories:");
    let mut stmt = conn.prepare("SELECT path FROM directories WHERE LOWER(path) LIKE ?1")?;
    let dirs = stmt.query_map(params![keyword], |row| row.get::<_, String>(0))?;
    for dir in dirs.flatten() {
        println!("📁 {}", dir);
    }

    println!("\n🔍 Matching files:");
    let mut stmt = conn.prepare("SELECT path, size, hash FROM files WHERE LOWER(path) LIKE ?1")?;
    let files = stmt.query_map(params![keyword], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?, row.get::<_, Option<String>>(2)?))
    })?;
    for file in files.flatten() {
        println!("📄 {} ({} bytes) {}", file.0, file.1, file.2.unwrap_or_default());
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conn = Connection::open("index.db")?;
    setup_database(&conn)?;

    match cli.command {
        Commands::Index { dir } => {
            println!("📂 Indexing: {}", dir);
            index_directory(&dir, &conn)?;
            println!("✅ Done.");
        }
        Commands::Search { name } => {
            search(&conn, &name)?;
        }
    }

    Ok(())
}

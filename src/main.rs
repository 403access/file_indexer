use rusqlite::{params, Connection};
use rusqlite::OptionalExtension;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use walkdir::WalkDir;

fn init_db(conn: &Connection) {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE,
            name TEXT,
            size INTEGER,
            modified REAL,
            hash TEXT
        )",
        [],
    ).unwrap();
}

fn hash_file(path: &Path) -> Option<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = fs::File::open(path).ok()?;
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(hasher.finalize().to_hex().to_string())
}

fn scan_folder(path: &str, conn: &Connection) {
    let mut new = 0;
    let mut updated = 0;
    let mut skipped = 0;

    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let metadata = match fs::metadata(path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };

        let modified = match metadata.modified() {
            Ok(time) => time
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            Err(_) => continue,
        };

        let path_str = path.to_string_lossy().to_string();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let size = metadata.len();

        let mut stmt = conn
            .prepare("SELECT modified FROM files WHERE path = ?1")
            .unwrap();
        let existing = stmt
            .query_row([&path_str], |row| row.get::<_, f64>(0))
            .optional()
            .unwrap();

        if let Some(existing_mod) = existing {
            if (existing_mod - modified).abs() < f64::EPSILON {
                skipped += 1;
                continue;
            } else {
                conn.execute("DELETE FROM files WHERE path = ?1", [&path_str])
                    .unwrap();
                updated += 1;
            }
        } else {
            new += 1;
        }

        let file_hash = hash_file(path).unwrap_or_else(|| "ERROR".to_string());

        conn.execute(
            "INSERT INTO files (path, name, size, modified, hash) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path_str, name, size, modified, file_hash],
        )
        .unwrap();
    }

    println!("\nScan complete:");
    println!("New: {new}, Updated: {updated}, Skipped: {skipped}");
}

fn search(conn: &Connection, term: &str) {
    let mut stmt = conn
        .prepare("SELECT path FROM files WHERE name LIKE ?1")
        .unwrap();

    let query = format!("%{}%", term);
    let rows = stmt
        .query_map([query], |row| row.get::<_, String>(0))
        .unwrap();

    for result in rows {
        println!("{}", result.unwrap());
    }
}

fn main() {
    let conn = Connection::open("file_index.db").unwrap();
    init_db(&conn);

    println!("Enter folder to scan:");
    let mut folder = String::new();
    io::stdin().lock().read_line(&mut folder).unwrap();
    let folder = folder.trim();

    if !Path::new(folder).is_dir() {
        eprintln!("Not a valid folder.");
        return;
    }

    scan_folder(folder, &conn);

    loop {
        println!("\nSearch by filename (Enter empty to quit):");
        let mut term = String::new();
        io::stdin().lock().read_line(&mut term).unwrap();
        let term = term.trim();
        if term.is_empty() {
            break;
        }
        search(&conn, term);
    }
}

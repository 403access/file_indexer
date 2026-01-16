My Goal:
- find all duplicate files and delete them
- make sure to organize the files and try to keep the already organized one

# 🦀 Rust File Indexer

A minimal and fast local file indexer written in Rust.

## ✅ Features

- 🔁 Incremental scan (skips unchanged files)
- 💾 SQLite database storage with unique constraints
- 🔍 Search by filename using LIKE queries
- ⚡ Fast hashing with Blake3
- 🧱 Cross-platform (Windows, macOS, Linux)
- 🧪 Easily extendable (tagging, content indexing, UI)

## 📄 Stored Metadata

| Field    | Description            |
| -------- | ---------------------- |
| path     | Full absolute path     |
| name     | Filename only          |
| size     | File size in bytes     |
| modified | UNIX timestamp (float) |
| hash     | BLAKE3 content hash    |

## 📦 Build & Run

```bash
cargo run --release

cargo build --release
./target/release/file_indexer
```

## 🧪 Tests

```bash
cargo test
```

## High level tasks
- Traverse given directory
- Store all files and directories in database
    - file name is a separate table "file_names"
    - "files" table contains the following columns:
      - file_name_id
      - file_kind: either "file" or "folder"
      - traversed: boolean (true is default value) / timestamp (null is default value)
        - some directories like .git, .next, node_modules und vendors should be skipped
        - directories can be traversed at any time later
      - size: file size in bytes
      - modified: timestamp or default null? Not sure about that
      - hash: whatever makes sense
      - 
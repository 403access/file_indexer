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

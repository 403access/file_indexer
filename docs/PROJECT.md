# file_indexer

A local file indexing and duplicate detection tool written in Rust.

## What It Does

file_indexer scans a directory tree, stores file metadata in a SQLite database, and enables searching and duplicate detection across indexed files. The long-term goal is to help users identify and clean up duplicate files and consolidate similar directory structures.

## Product Vision

The tool targets users who need to manage large file collections — finding duplicates, comparing folder structures, and cleaning up redundant copies. The planned UI is a web-based diffing view that shows side-by-side directory comparisons with status indicators for matching, missing, and modified files.

## Tech Stack

| Component | Technology |
|---|---|
| Language | Rust (2021 edition) |
| Database | SQLite via `rusqlite` |
| Hashing | SHA-256 via `sha2` (partial-file fast hash) |
| Web Server | `axum` (HTTP framework) |
| Async Runtime | `tokio` |
| CLI Prompts | `inquire` |
| Progress Bars | `indicatif` |
| Config | `dotenvy` (.env files) |
| Serialization | `serde` (JSON) |
| Directory Traversal | `std::fs::read_dir` (recursive) |
| Logging | `tracing-subscriber` |
| Test Utilities | `uuid` (temp directory naming) |

## Architecture

```
main.rs                          Entry point (Axum server)
  ├── environment/               Load .env config (CWD, DATABASE_URL)
  ├── states/                    Thread-local app state
  ├── commands/                  Interactive REPL (currently disabled)
  │     └── services/            Business logic layer
  │           └── sql/           Database queries (search, duplicates, CRUD)
  │                 └── file_entry/   Data types, conversion, hashing
  └── api/                       Axum HTTP handlers (placeholder)
```

## Project Status

The project is in a **transitional phase**. A fully functional CLI REPL (interactive mode with `inquire`) has been built but is now disabled in `main.rs`. The replacement Axum HTTP server is scaffolded with only placeholder routes. The core database and file processing logic works, but the API layer and several feature areas are incomplete.

## Feature Roadmap

### Indexing

| Feature | Status | Notes |
|---|---|---|
| Recursive directory traversal | Done | Uses `std::fs::read_dir` |
| Store files in SQLite | Done | Tables: `files`, `file_names` with unique constraints |
| Store directories in SQLite | Done | `is_directory` flag on files table |
| SHA-256 content hashing | Done | Fast partial hash (size + first/last 4KB) |
| File metadata (path, name, size, modified) | Done | Stored per file entry |
| Symlink detection | Done | `is_symlink` field in schema |
| Transaction-based inserts | Done | Bulk insert within DB transactions |
| Skip special directories (.git, node_modules) | Not started | Mentioned in README, not implemented |
| Index file creation date | Not started | `created` field exists in FileEntry but not populated during indexing |
| Parent directory foreign key | Not started | Planned as separate table |
| Child count for folders | Not started | Mentioned in README |
| Incremental scan (skip unchanged) | Not started | README claims this, not implemented |

### Search

| Feature | Status | Notes |
|---|---|---|
| Search by filename | Done | SQL LIKE queries with pattern support |
| Pattern types (Exact/StartsWith/EndsWith/Contains) | Done | Enum-driven pattern matching |
| Filter by kind (Files/Folders/Both) | Done | `TargetKind` enum |
| Sort results (ASC/DESC, case-sensitive/insensitive) | Done | `SortOrder` enum |
| Service layer for search | Done | `search_service.rs` wraps DB calls |
| API endpoint for search | Not started | No HTTP route exists yet |

### Duplicate Detection

| Feature | Status | Notes |
|---|---|---|
| SQL query for duplicate hashes | Done | Groups files by hash, returns duplicates |
| Duplicate hash table management | Done | Create/reset `duplicate_hashes` table |
| Duplicate service layer | Not started | `duplicate_service.rs` returns "Not implemented yet" |
| CLI command for indexing duplicates | Not started | Stub only (sleep + fake logs) |
| CLI command for listing duplicates | Not started | Stub only (sleep + fake logs) |
| API endpoint for duplicates | Not started | No HTTP route exists yet |
| Filter duplicates by name/extension/size | Not started | Mentioned in README |
| Sort by occurrence count | Not started | Mentioned in README |

### Directory Comparison / Consolidation

| Feature | Status | Notes |
|---|---|---|
| Design for directory similarity detection | Partial | Extensive design comments in `duplicates.rs` |
| Folder hash/comparison table | Not started | Mentioned in design notes |
| Diffing view (web UI) | Not started | Planned HTML tree view with side-by-side comparison |
| Detect renamed folders | Not started | Mentioned in README diffing view |
| Detect modified files across copies | Not started | Core goal, not started |

### Cleanup

| Feature | Status | Notes |
|---|---|---|
| Delete duplicate files | Not started | Core goal, not started |
| Skip directories (node_modules) | Not started | Listed in features.md |
| Preserve organized files | Not started | Mentioned in README goals |

### Infrastructure

| Feature | Status | Notes |
|---|---|---|
| Axum HTTP server | Scaffolded | Only placeholder routes (Hello World, hardcoded user) |
| REPL mode | Built but disabled | Commented out in `main.rs` |
| CLI argument parsing | Partial | `check_arguments.rs` works; `clap` used in prototype only |
| Environment variable config | Done | .env loading via dotenvy |
| Thread-local app state | Done | `app_state` module |
| Error handling patterns | In progress | `docs/rust_error_handling.md` exists as reference |
| Logging | Scaffolded | `tracing-subscriber` imported, not fully configured |
| Tests | Minimal | 1 integration test for DB init; no search/duplicate/API tests |
| Documentation | Partial | README and features.md exist but are out of sync with code |

## Known Issues

1. **`command_init_db.rs`** — `_init_db()` always returns an error even after successful initialization. The error is silently swallowed.
2. **Hardcoded DB path** — `command_index_files.rs` and `search_service.rs` hardcode `"file_index.db"` instead of using `app_state::get_db()`.
3. **README claims Blake3** — The README mentions Blake3 hashing, but the code uses SHA-256 via the `sha2` crate.
4. **Unused duplicate `FileEntry`** — `index_files/_types.rs` defines a redundant struct not used anywhere.
5. **Empty files** — `arguments_check.rs`, `_config.rs`, and `index_service.rs` are empty placeholders.
6. **`ai-main.rs`** — A standalone prototype file with its own `main()` that is not part of the crate compilation.

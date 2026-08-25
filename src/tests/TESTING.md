# Testing Guide

## Overview

All tests live under `src/tests/` as **in-crate unit tests**, conditionally compiled via `#[cfg(test)]` in `lib.rs`. There are no standard Rust integration tests (no top-level `tests/` directory).

## Directory Structure

```
src/
├── lib.rs                              # #[cfg(test)] pub mod tests;
└── tests/
    ├── mod.rs                          # pub mod database;
    ├── database/
    │   ├── mod.rs                      # #[test] actual_test()
    │   └── arrange.rs                  # Setup/teardown helpers
    └── data/
        └── sample-directory/           # Static fixture directory tree
```

## Test Pattern: Arrange-Act-Assert

Tests follow a manual **AAA (Arrange-Act-Assert)** pattern. Currently only the **Arrange** phase has its own module; Act and Assert are inlined in the test function.

### Arrange (`arrange.rs`)

Provides setup and teardown utilities:

| Function | Purpose |
|---|---|
| `database_exists() -> bool` | Checks if a test database file exists at the hardcoded path |
| `create_database(path) -> PathBuf` | Calls `_init_db` to initialize a SQLite database at the given path |
| `delete_database(path)` | Removes the database file |
| `create_temp_folder() -> PathBuf` | Creates a unique temp directory under the OS temp dir using a UUID v4 name |
| `create_file(dir, name)` | Creates a file where the filename equals its content |
| `create_test_data() -> PathBuf` | Builds a full directory tree in a temp folder (see structure below) |
| `delete_test_data(path)` | Removes the temporary test data folder |

### Test Data Structure

`create_test_data()` programmatically builds this tree in a temp directory:

```
<uuid>/
├── file-a
├── folder-a/
│   ├── file-a-1
│   └── file-a-2
├── folder-b/
│   └── file-b-1
├── folder-c/
│   ├── file-a-1
│   ├── file-c-1
│   ├── file-c-2
│   ├── file-c-3
│   └── folder-d/
│       ├── folder-a/
│       │   ├── file-a-1
│       │   └── file-a-2
│       └── folder-b/
│           └── file-b-1
├── folder-e/
│   ├── folder-a/
│   │   ├── file-a-1
│   │   └── file-a-2
│   ├── folder-b/
│   │   └── file-b-1
│   └── folder-c/
│       ├── file-c-1
│       └── file-c-2
└── folder-f/
    ├── file-a
    ├── file-b
    └── file-f
```

### Act + Assert

The test function in `database/mod.rs` runs the arrange helpers, then (currently) the act of creating the database IS the assertion -- `create_database` calls `_init_db` and asserts success internally.

## Running Tests

```sh
cargo test
```

## Why This Test Data Structure?

The directory tree intentionally uses **overlapping folder and file names at multiple levels** -- `folder-a`, `folder-b`, `folder-c` and files like `file-a-1`, `file-b-1` appear in different branches and at different depths. This ensures the indexer correctly tracks **which file lives in which directory**, not just that it can find files by name.

## Key Design Decisions

- **In-crate tests** rather than integration tests, giving access to private items via `pub mod tests` in `lib.rs`.
- **Dynamic test data** created at runtime in temp directories with UUID-based paths for isolation. The static `data/sample-directory/` fixture exists but is not currently referenced by any test.
- **No third-party test crates** -- only `std` assertions (`assert!`), `uuid` for temp paths, and `tokio` for async bridging.
- **Mixed sync/async** -- `std::futures::executor::block_on` bridges sync test code with async `tokio::fs` calls in `arrange.rs`.

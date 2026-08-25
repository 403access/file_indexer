# Test Coverage Analysis

This document outlines what tests exist today, what is missing, and what should be built next to achieve complete coverage of the file indexer's functionality.

## Current Test Suite

The test suite contains exactly **one test** (`actual_test`) that validates database initialization and cleanup. It does not assert that the database schema is correct or that any data can be stored/retrieved.

| Test | What It Covers |
|---|---|
| `actual_test` | Creates a temp folder, creates a DB via `_init_db`, cleans up |

## What Is Missing

### 1. File Entry Conversion (`file_entry/convert.rs`)

The bridge between the filesystem and the data model. No tests exist for:

- **`convert_from_dir`** -- that a `DirEntry` produces a correct `FileEntry` with the right name, path, size, and type flags (`is_file`, `is_directory`, `is_symlink`)
- **`compute_sha256_fast`** -- that the partial hash (size + first/last 4KB) is deterministic for the same file, and that different files produce different hashes
- **`convert_from_row` / `convert_from_rows`** -- that SQLite rows round-trip back into correct `FileEntry` structs with all fields preserved

### 2. Sorting (`file_entry/sort.rs`)

Three sort modes exist but none are tested:

- **`Default`** -- natural ordering by the `Ord` derive
- **`ABCabc`** -- case-sensitive name sort
- **`AaBbCc`** -- case-insensitive name sort

Each should be tested with a list of entries containing mixed-case names.

### 3. Database Operations (`sql/database.rs`)

The core persistence layer has no tests:

- **Schema creation** -- `files` and `file_names` tables are created with correct columns and types
- **Indexes** -- `idx_files_hash` and `idx_files_path` exist after init
- **`insert_file_name`** -- inserts correctly, returns the right ID, ignores duplicates
- **`insert_file`** -- inserts correctly, foreign key to `file_names` works, returns the right ID, ignores duplicates

### 4. Search (`sql/search.rs`)

The search engine has four pattern modes, type filtering, and sort direction -- none tested:

- **Exact** -- matches only the full name
- **StartsWith** -- matches names beginning with the pattern
- **EndsWith** -- matches names ending with the pattern
- **Contains** -- matches names containing the pattern
- **Type filtering** -- Files only, Folders only, Both
- **Sort direction** -- ASC and DESC ordering

### 5. Duplicate Detection (`sql/duplicates.rs`)

The SQL layer is implemented but untested:

- **`create_duplicates_table`** -- files with the same hash are grouped
- **`get_duplicates`** -- returns all files sharing a hash, respects the `limit` parameter
- **No duplicates** -- returns an empty result when all hashes are unique

### 6. Indexing Logic (`commands/command_index_files.rs`)

The recursive BFS traversal is the most complex feature and has no tests:

- **Recursive traversal** -- all files and directories in the test tree are indexed
- **Type flags** -- directories marked as directories, files as files
- **Path correctness** -- stored paths match the actual filesystem paths
- **Name correctness** -- stored names match the actual filenames
- **File metadata** -- size, modified timestamp, hash are stored correctly

### 7. Filesystem Reading (`search_files/try_get_dir_entries.rs`)

No tests for reading a single directory:

- **Valid path** -- returns correct entries for a known directory
- **Invalid paths** -- rejects empty strings, relative paths, non-existent paths

### 8. Services (`services/`)

The service layer that ties commands to database operations is untested:

- **`search_service`** -- end-to-end: insert data, search, get correct results
- **`duplicate_service`** -- currently a stub, but when implemented needs end-to-end tests
- **`index_service`** -- currently empty, but when implemented needs end-to-end tests

## Priority Order

1. **File entry conversion** -- foundational, everything depends on it
2. **Database operations** -- schema correctness and insert logic
3. **Search** -- the most user-facing feature
4. **Duplicate detection** -- the core value proposition
5. **Indexing logic** -- the most complex operation
6. **Filesystem reading** -- simple but needs validation
7. **Sorting** -- low risk but should be covered
8. **Services** -- end-to-end once unit tests exist

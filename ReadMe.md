Goal:
- Delete duplicate files
- Resolve difference of similar directories

Tasks
- Index all files
  - File Path
  - Directory Path
  - Name
  - File Size
  - File Creation Date (-Time)
  - File Modification Date (-Time)
  - Custom Hash
  - Child count (if a folder)
  - Database related
    - parent directory index number (separate table)
- Find duplicate files
- Show duplicate files
  - Sort by highest amount of occurrences
  - Filter files by
    - name
    - extension
    - file size
  - Show similar file and folder structures:
    
    ~WHAT~
    Sometimes the same files and folders are copied to multiple locations.
    Some of those copied items can then be modified by changing their content.

    ~HOW~
    There are essentially two scenarios.

    Scenario 1:
    This might be the faster scenario since we start by looking at folders first.
    ...

    Scenario 2:
    For each file

    ~Notes~
    The same way we have a table for files with their hashes that are used as foreign keys
    within other tables referencing those files, we need to have a table folders

    ~Features~
    - Diffing View
    Show a tabled tree view (in html) similar to:

```
    Name                    Version a             Version b
                            Path                  Path

    |-sample-directory      /sample-directory
    |  |-file-a             ✅ /file-a                 🚫 -
    |  |-folder-c           ✅ /folder-c               ⚠️ /renamed-folder-c
    |  |  |-file-c-1
    |  |  |-folder-d
    |  |  |  |-folder-b
    |  |  |  |  |-file-b-1
    |  |  |  |-folder-a
    |  |  |  |  |-file-a-2
    |  |  |  |  |-file-a-1
```

- Delete duplicate files

My Goal:
- find all duplicate files and delete them
- make sure to organize the files and try to keep the already organized ones

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

## 🏗️ Setup

### Environment Variables file

- Copy and paste `.example.env`
- Rename the newly created file to `.env`
- Make sure the variables are set properly.

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

## 📋 High level tasks
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
- better logging
  - https://docs.rs/env_logger/latest/env_logger/

## 🧰 Tools

### AI

[Gemini](https://github.com/google-gemini/gemini-cli)

```
npx @google/gemini-cli
```


find ./data | sed -e "s/[^-][^\/]*\//  |/g" -e "s/|\([^ ]\)/|-\1/"
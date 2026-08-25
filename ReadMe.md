# 🦀 File Indexer

A fast local file indexer written in Rust. Index terabyte-sized archives into
SQLite, find exact duplicates and near-duplicate folder trees, and manage
everything through a built-in web UI.

> **Goal:** find all duplicate files, delete them safely, and resolve the
> differences between similar directories — while keeping already-organized
> folders intact.

## ✨ Features

### Indexing

- 🔁 **Incremental scan** — folders with an unchanged mtime are skipped, so
  re-indexing after startup resumes almost instantly
- 💾 **SQLite storage** — WAL mode, indexed lookups, normalized file names
- ⚡ **Sampled SHA-256 hashing** — size + first/last 4 KB per file, so
  multi-gigabyte files don't need full reads
- 🚫 Ignore rules for `.git`, `node_modules`, etc., with skipped-path tracking

### Duplicate detection

| Level | What it finds | How |
|-------|---------------|-----|
| Files | Identical content across the whole index | Shared-hash lookup (`duplicate_hashes`) |
| Folders | Folders whose entire content is duplicated | Union-find over shared duplicate files |
| Near-duplicate folders | Folder trees sharing 80–99% of their content despite renames, edits, additions or deletions | Inverted index + MinHash signatures + banded LSH, verified with exact Jaccard similarity |

Near-duplicate detection scales past O(N²) pairwise comparison: candidate
pairs come from LSH band collisions (large corpora) or exact posting-list
walks (small corpora), and only survivors are verified. Generic noise files
(`.DS_Store`, `package-lock.json`, `__init__.py`, …) are excluded from folder
signatures so they can't inflate similarity scores.

For every near-duplicate pair the UI reports a concrete delta:
files only in A, only in B, changed (same name, different hash), identical.

### Web UI

Light / dark / system themes, sidebar or topbar layout:

- **Dashboard** — live index statistics
- **Search** — filename search
- **Explorer** — browse the indexed tree, with duplicate flags per entry
- **Duplicates → Files / Folders / Near-Dupes** — three levels of dedup views
- **Processes** — monitor every background job with pause/resume/stop/trigger
- **Logs / Skipped / Ignored / Settings**

Background processes run on configurable intervals and persist their
"stopped by user" state in the database across restarts.

## 📄 Stored metadata

| Field      | Description                        |
| ---------- | ---------------------------------- |
| path       | Full absolute path                 |
| parent     | Parent directory path              |
| name       | Filename (normalized in its own table) |
| size       | File size in bytes                 |
| modified   | UNIX timestamp                     |
| hash       | Sampled SHA-256 content hash       |
| traversed  | Whether a directory was fully scanned |

## 🏗️ Setup

```bash
git clone <repo>
cd file_indexer
cp example.env .env   # then edit .env
```

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CWD` | current directory | Directory to index |
| `PORT` | `3000` | HTTP server port |
| `DATABASE_URL` | derived from `CWD` | SQLite database path |
| `ENABLE_STARTUP_INDEXING` | `true` | Index automatically at startup |
| `ENABLE_INITIAL_DASHBOARD_REFRESH` | `true` | One-time dashboard refresh at startup |
| `ENABLE_DASHBOARD_REFRESH` | `true` | Periodic dashboard stats refresh |
| `ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH` | `true` | Materialize duplicate folder groups |
| `DUPLICATE_FOLDER_GROUPS_REFRESH_INTERVAL` | `120` | Interval in seconds |
| `ENABLE_NEAR_DUPLICATE_FOLDERS_REFRESH` | `true` | MinHash/LSH near-duplicate scan |
| `NEAR_DUPLICATE_FOLDERS_REFRESH_INTERVAL` | `300` | Interval in seconds |
| `NEAR_DUPLICATE_MIN_SIMILARITY` | `0.8` | Jaccard threshold (0.05–1.0) |
| `IGNORE_PROCESS_DATABASE_STATE` | `false` | Start jobs even if stopped in DB |

**Indexing-only mode** (e.g. first ingest of a huge archive):

```env
ENABLE_STARTUP_INDEXING=true
ENABLE_INITIAL_DASHBOARD_REFRESH=false
ENABLE_DASHBOARD_REFRESH=false
ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH=false
ENABLE_NEAR_DUPLICATE_FOLDERS_REFRESH=false
IGNORE_PROCESS_DATABASE_STATE=true
```

See [docs/NIXOS.md](docs/NIXOS.md) for running it as a NixOS systemd service.

## 📦 Build & Run

```bash
cargo build --release
./target/release/file_indexer
```

Then open `http://localhost:3000`.

## 🔌 API

Selected endpoints (all JSON):

```
GET  /api/dashboard                      GET  /api/duplicates
GET  /api/search?q=                      GET  /api/duplicate-folders
GET  /api/folder?path=                   GET  /api/near-duplicate-folders
GET  /api/explorer?path=                       ?min_similarity=&max_similarity=&q=
GET  /api/skipped                        GET  /api/near-duplicate-folders/delta?a=&b=
GET  /api/processes                      POST /api/processes/{id}/trigger
GET  /api/logs                           POST /api/processes/types/{key}/enable
GET  /api/settings                       GET  /api/status
```

## 🎨 Web UI & design system

Static assets live under `static/` — no frontend bundler.

| Doc | Contents |
|-----|----------|
| [docs/UI.md](docs/UI.md) | Themes, tokens, components, Drawer API, page checklist |
| [docs/PROJECT.md](docs/PROJECT.md) | Architecture and product overview |
| [docs/NIXOS.md](docs/NIXOS.md) | NixOS deployment guide |

Quick conventions:

- Link `tokens.css` → `components.css` → `style.css` on every page
- Prefer CSS variables (`var(--accent)`) over hardcoded colors
- Use shared classes (`.btn`, `.card`, `.page-header`, `.table-wrap`, …)
- Inject nav via `#sidebar-container` + `sidebar.js` (do not copy-paste menus)
- Prefer `Drawer.create(…)` for new detail side panels

## 🧪 Tests

```bash
cargo test
```

Includes unit tests for the MinHash/LSH engine (Jaccard math, LSH recall,
threshold filtering), SQL layers, and process management.

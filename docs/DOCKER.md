# Docker

Run the test suite inside a container to isolate it from the host system.

## Quick Start

```bash
docker compose up --build
```

This builds the image and runs `cargo test`. All 69 tests execute inside the container.

## Useful Commands

| Command | What it does |
|---|---|
| `docker compose up --build` | Build image + run tests |
| `docker compose build` | Build image only |
| `docker compose run test cargo test -- --nocapture` | Run tests with stdout visible |
| `docker compose run test sh` | Open a shell inside the container |

## Base Image

The Dockerfile uses `rust:1.90-alpine` — a minimal Alpine-based image (~350MB vs ~1GB for Debian-based `rust:1.90`). This significantly reduces the attack surface and Docker vulnerability count.

### Why Alpine?

The Debian-based `rust:1.90` image ships with 2 critical and 89 high CVEs from inherited system packages. Alpine's minimal footprint means fewer packages and far fewer known vulnerabilities.

### Why `rusqlite` with `bundled`?

Alpine uses **musl** libc instead of **glibc**. Without the `bundled` feature, `rusqlite` links against the system SQLite library, which causes segfaults on musl due to ABI mismatches.

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

This compiles SQLite from C source via the `cc` crate, embedding it directly into the binary. The tradeoffs:

| | Without `bundled` | With `bundled` |
|---|---|---|
| Build speed | Faster | ~5-10s slower |
| Binary size | Smaller | ~1-2MB larger |
| Runtime deps | Needs `libsqlite3` | Self-contained |
| Portability | Breaks on musl/Alpine | Works everywhere |
| SQLite version | OS-provided | Pinned (3.45.x, well-audited) |

## Files

| File | Purpose |
|---|---|
| `Dockerfile` | Defines the test container |
| `docker-compose.yml` | Orchestration config |
| `.dockerignore` | Excludes `target/`, `.DS_Store`, `.env`, `*.db`, `.git/` from build context |

# Running file_indexer on NixOS

This guide covers running the app on a NixOS machine, optimized for
indexing large amounts of data (terabytes) with all other background
jobs disabled.

## 1. Configure the environment

Create a `.env` file next to the project (or anywhere persistent, e.g.
`/var/lib/file-indexer/.env`) based on `example.env`:

```env
# Directory containing the terabytes of data to index
CWD=/mnt/data

# Port for the HTTP API server (the server always binds; see notes)
PORT=3000

# Only indexing should run — disable everything else
ENABLE_STARTUP_INDEXING=true
ENABLE_INITIAL_DASHBOARD_REFRESH=false
ENABLE_DASHBOARD_REFRESH=false
ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH=false

# Ignore any "stopped by user" flags persisted in the database,
# so indexing is never skipped on startup
IGNORE_PROCESS_DATABASE_STATE=true

# Keep the SQLite database somewhere persistent
DATABASE_URL=/var/lib/file-indexer/file_index.db
```

Notes:

- `IGNORE_PROCESS_DATABASE_STATE=true` ensures a previously "stopped by
  user" flag in the DB will not block startup indexing.
- The HTTP server still binds to `PORT` (hardcoded in `src/main.rs`);
  it is required for the process to run. With all refresh jobs disabled
  no heavy background work happens as long as the UI is not used.

## 2. Quick run (manual)

For a one-off test without installing anything globally:

```bash
nix-shell -p rustc cargo pkg-config sqlite

cargo build --release
./target/release/file_indexer
```

The `.env` must be present in the working directory.

## 3. Proper systemd service (recommended)

Create `/etc/nixos/file-indexer-service.nix`:

```nix
{ config, lib, pkgs, ... }:
{
  systemd.services.file-indexer = {
    description = "File Indexer";
    after = [ "network.target" ];
    wantedBy = [ "multi-user.target" ];

    serviceConfig = {
      ExecStart = "${
        pkgs.rustPlatform.buildRustPackage {
          pname = "file_indexer";
          version = "0.1.0";
          src = /path/to/file_indexer;
          cargoLock.lockFile = /path/to/file_indexer/Cargo.lock;
        }
      }/bin/file_indexer";
      EnvironmentFile = /var/lib/file-indexer/.env;
      WorkingDirectory = /path/to/file_indexer; # needed so ./static is found
      Restart = "on-failure";
    };
  };
}
```

Add it to your `configuration.nix`:

```nix
imports = [ ./file-indexer-service.nix ];
```

Rebuild and start:

```bash
sudo nixos-rebuild switch
sudo systemctl status file-indexer
```

## 4. Important details

- **WorkingDirectory matters**: the app serves `static/` relative to its
  working directory (`src/main.rs`), so point it at the checked-out
  project folder.
- **Database location**: `DATABASE_URL` from `.env` controls where the
  SQLite files land. Use a persistent path like `/var/lib/file-indexer/`
  so the index survives rebuilds and reboots.
- **First build takes a while** since it compiles tokio/axum and friends.
- **Logs**: check with `journalctl -u file-indexer -f`.

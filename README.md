# mlocate

A high-performance, metadata-aware alternative to GNU `locate` built with Rust and Roaring Bitmaps.

## Features

- **Fast substring search** — Trigram-accelerated bitmap intersection with LIKE verification
- **Metadata-aware** — Search by file size, modification time, and MIME type
- **Rich output** — Modern table view, JSON, plain text, or NUL-terminated
- **GNU compatible** — `--gnu` mode accepts standard `locate` flags
- **Parallel indexing** — Multi-threaded filesystem crawler with MIME detection
- **Atomic index swap** — Crash-safe database updates
- **Auto-indexing** — Built-in cron/LaunchAgent scheduling via `--install-cron`

## Quick Start

```bash
# Index your files
mupdatedb --localpaths /path/to/scan

# Search
mlocate myfile
mlocate -i "*.rs" --json
mlocate --size 10MB+ --modified 7d- "report"
```

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/mlocate --help
```

## Components

| Binary | Description |
|--------|-------------|
| `mlocate` | Search the indexed database with filters and formatting |
| `mupdatedb` | Build/update the file index database |

## License

MIT

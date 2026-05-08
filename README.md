# mlocate

A high-performance, metadata-aware alternative to GNU `locate` built with Rust and Roaring Bitmaps.

## Features

### Search
- **Fast trigram indexing** — Trigram-accelerated bitmap intersection; bigram fallback for 2-char patterns
- **Regex-aware** — Regex patterns extract literals for trigram pre-filtering, avoiding full scans
- **Metadata filters** — Search by file size, modification time, and MIME type (exact or glob)
- **Streaming output** — Results streamed directly to stdout (no in-memory buffering)
- **Rich output** — Modern table view, JSON, plain text, or NUL-terminated

### Indexing (File Format v2)
- **128-byte header** with feature flags, directory table offsets, and bitmap metadata
- **Directory table** — Binary-searchable directory entries for `--incremental` updates
- **Extension index** — Per-extension bitmaps for fast MIME-type filtering
- **Bigram index** — 2-char pattern matching without full table scans
- **CRC32 integrity** — Footer checksum validated on open; detects corruption and truncation
- **Parallel indexing** — Multi-threaded filesystem crawler with optional magic-byte MIME detection
- **Incremental updates** — Skips unchanged directories via stored mtime comparison
- **Atomic index swap** — Writes to `.tmp` then atomically renames; crash-safe

### Architecture
- **Roaring compressed bitmaps** for memory-efficient document ID sets
- **Memory-mapped I/O** via `memmap2` with SIGBUS handler for graceful crash recovery
- **External merge sort** — Chunked writes to disk, K-way merged via binary heap for constant memory indexing
- **Arc-based caching** — Shared bitmap references avoid repeated cloning in multi-pattern searches
- **Batch trigram lookup** — Single directory traversal fetches all needed trigrams at once

## Quick Start

```bash
# Index your files
mupdatedb --localpaths /path/to/scan
mupdatedb --localpaths /home    # with --no-magic-mime for speed

# Search
mlocate myfile
mlocate -i "*.rs" --json
mlocate --size 10MB+ --modified 7d- "report"
mlocate -r '\.rs$'              # regex (trigram-accelerated via literal extraction)

# Inspect
mlocate --statistics             # database stats as JSON
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
| `mlocate` | Search the indexed database with filters, regex, and formatted output |
| `mupdatedb` | Build/update the file index database (parallel crawl + merge sort) |

### Index file layout (v2)

| Section | Description |
|---------|-------------|
| Header (128 bytes) | Magic bytes, version, feature flags, section offsets |
| Config (zstd JSON) | Indexed/pruned paths, hostname, timestamp |
| File directory | Path, size, mtime, mode, MIME type per file |
| File offset directory | 8-byte offsets into the file directory (binary search compat) |
| Trigram directory | Sorted `[3-byte key][8-byte offset][4-byte length]` entries |
| Bigram directory | Sorted `[2-byte key][8-byte offset][4-byte length]` entries |
| Extension directory | Sorted `[8-byte key][8-byte offset][4-byte length]` entries |
| Dir table | Sorted directory entries (path, mtime, ino, file range) |
| Bitmap data | Roaring-compressed document ID bitmaps |
| CRC32 footer (4 bytes) | Integrity checksum of all preceding bytes |

## License

MIT

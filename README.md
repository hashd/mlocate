# mlocate

[![CI](https://github.com/hashd/mlocate/actions/workflows/ci.yml/badge.svg)](https://github.com/hashd/mlocate/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, metadata-aware alternative to GNU `locate` — built in Rust with Roaring bitmaps.

```bash
$ mupdatedb --localpaths ~/projects
$ mlocate budget.xlsx
/Users/kiran/projects/finance/budget.xlsx

$ mlocate -i -r readme --json
[{"path":"/home/user/README.md","size":2048,"mtime":1715200000,...}]

$ mlocate --size 1MB+ --modified 7d- -t "image/*" screenshot
```

## Features

**Search**
- Trigram-accelerated bitmap intersection for fast path lookups
- Regex support with automatic literal extraction for trigram pre-filtering
- Filter by file size (`--size 10MB+`), modification time (`--modified 7d-`), and MIME type (`-t image/*`)
- Streaming output — results go straight to stdout, no buffering
- Multiple output formats: JSON, table, plain text, NUL-terminated

**Indexing**
- Parallel filesystem crawler with optional magic-byte MIME detection
- External merge sort — constant-memory indexing via disk chunks and k-way merge
- Incremental updates via directory mtime comparison
- CRC32 integrity checks, atomic `.tmp` → rename, crash-safe writes
- Memory-mapped reads with SIGBUS handling for graceful recovery

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/mlocate --help
```

## Usage

```bash
# Build an index
mupdatedb                          # defaults: /Users (macOS), /home /etc /usr /opt (Linux)
mupdatedb --localpaths ~/projects  # index specific directories
mupdatedb --force                  # full rebuild

# Search
mlocate readme                    # literal substring match
mlocate -i README                 # case-insensitive
mlocate -b readme                 # match filename only
mlocate -r '\.rs$'                # regex (trigram-accelerated)
mlocate -c "*.log"                # count matches
mlocate -0 "*.tmp" | xargs -0 rm  # pipe to xargs
```

**Filters**

```bash
mlocate --size 10MB+ report        # >= 10 MB
mlocate --size 1KB- notes          # <= 1 KB
mlocate --modified 7d- config      # modified within 7 days
mlocate --modified 2h+ log         # older than 2 hours
mlocate -t text/plain doc          # exact MIME type
mlocate -t "image/*" screenshot    # MIME glob
```

See [USAGE.md](USAGE.md) for the full flag reference, filter syntax, shell completions, and exit codes.

## Database

| Platform | Default path |
|----------|-------------|
| macOS    | `~/Library/Caches/mlocate/mlocate.db` |
| Linux    | `~/.cache/mlocate/mlocate.db` |

Override with `--database` on either binary. Inspect with `mlocate --statistics`.

## GNU Compatibility

Pass `--gnu` to accept GNU `locate` flags (`-L`, `-A`, `-w`, `-P`, `-H`) with warnings instead of errors.

## Architecture

mlocate uses a custom binary format with Roaring-compressed inverted indexes. File paths are decomposed into trigrams at index time and stored in sorted directory entries for binary search. Queries intersect trigram bitmaps to find candidate files, then apply substring/regex post-filters, filters, and `--existing` checks — all streaming to stdout without intermediate collections. The index is memory-mapped for zero-copy reads and protected by a CRC32 footer and a lock file to prevent concurrent writes.

## License

MIT

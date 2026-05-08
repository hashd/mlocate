# mlocate Usage

## mupdatedb — Index Builder

```bash
mupdatedb [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--localpaths <PATHS>` | Directories to scan (repeatable). Default: `/Users` (macOS), `/home /etc /usr /opt` (Linux) |
| `--prunepaths <PATHS>` | Paths to exclude (repeatable). Default: `/Volumes /dev` (macOS), `/proc /sys /dev /run /tmp /var/tmp /snap /lost+found` (Linux) |
| `--incremental` | Skip subtrees with unchanged directory mtime (requires existing index; falls back to full rebuild on v1 indexes) |
| `--force` | Full reindex, ignoring existing database |
| `--dry-run` | Report what would be indexed without writing to database |
| `--parallel <N>` | Number of extractor worker threads (default: 4) |
| `--database <PATH>` | Override database file path |
| `--quiet` | Suppress progress output |
| `-v`, `--verbose` | Verbose output (file counts, timing) |
| `--no-magic-mime` | Skip magic-byte MIME detection for extensionless files. Faster indexing, but may lose MIME accuracy for files without extensions. |
| `--install-cron` | Install auto-index scheduling (LaunchAgent on macOS, systemd user timer on Linux) |
| `-V`, `--version` | Print version and exit |

### Examples

```bash
# Index home directory with defaults
mupdatedb

# Index specific paths, exclude node_modules
mupdatedb --localpaths /home/projects --prunepaths /home/projects/node_modules

# Quick incremental update
mupdatedb --incremental

# Fast indexing without magic-byte detection
mupdatedb --no-magic-mime

# Preview what would be indexed
mupdatedb --dry-run --localpaths /tmp/testdir

# Verbose output with 8 extractor threads
mupdatedb -v --parallel 8 --localpaths /home
```

---

## mlocate — Search

```bash
mlocate [OPTIONS] [PATTERNS...]
```

### Options

| Flag | Description |
|------|-------------|
| `-i`, `--ignore-case` | Case-insensitive matching with full Unicode case folding (via caseless, Unicode 12.0). Paths are overwhelmingly ASCII so this covers real-world use. |
| `-b`, `--basename` | Match pattern against filename only (the part after the last `/`) |
| `-r`, `--regex` | Treat pattern as regex. Literal substrings are extracted for trigram pre-filtering — non-trivial regexes stay efficient. |
| `-e`, `--existing` | Only show files currently on disk (validates each candidate path) |
| `-l`, `--limit <N>` | Cap results to N entries |
| `-c`, `--count` | Print match count instead of results |
| `-0`, `--null` | NUL-terminated output (for `xargs -0`) |
| `--size <FILTER>` | Filter by file size (see below) |
| `--modified <FILTER>` | Filter by modification time (see below) |
| `-t`, `--type <MIME>` | Filter by MIME type (exact or glob) |
| `--gnu` | GNU locate compatibility mode (accepts `-L`, `-A`, `-w`, `-P`, `-H` with warnings) |
| `-S`, `--statistics` | Print database stats as JSON. Aliases: `--schema`, `--stats`. |
| `--database <PATH>` | Override database file path |
| `--table` | Force table output |
| `--json` | JSON array output (streamed — no in-memory buffering) |
| `--plain` | Plain path-per-line output |
| `--icons` | Show Nerd Font file type icons. **Requires a Nerd Font installed and configured in your terminal.** |
| `--color <MODE>` | Color: `auto`, `always`, `never` (default: `auto`) |
| `-v`, `--verbose` | Verbose output |
| `-V`, `--version` | Print version and exit |
| `-h`, `--help` | Print help |
| `--generate-completions <SHELL>` | Generate shell completions (`bash`, `zsh`, `fish`) |

### Output Format

Output is auto-detected based on stdout:
- **TTY**: Modern table view (Path, Size, Modified) via pager (buffered for column alignment)
- **Pipe**: Plain text, one path per line (streamed)
- Override with `--table`, `--json`, `--plain`, or `-0`/`--null`

### Filter Syntax

**Size filter** — `--size <VALUE><UNIT><SUFFIX>`

| Unit | Example | Meaning |
|------|---------|---------|
| `B` | `500B` | 500 bytes |
| `KB` | `10KB` | 10,000 bytes |
| `MB` | `10MB+` | 10 MB or larger |
| `GB` | `1GB-` | 1 GB or smaller |

Suffix: `+` (>=), `-` (<=), omit for exact match.

**Time filter** — `--modified <VALUE><UNIT><SUFFIX>`

| Unit | Example | Meaning |
|------|---------|---------|
| `m` | `30m` | 30 minutes ago |
| `h` | `2h+` | 2 hours ago or earlier |
| `d` | `7d-` | Within the last 7 days |
| `w` | `1w+` | 1 week ago or earlier |

Suffix: `+` (older than), `-` (newer than), omit for exact match.

Note: the `+`/`-` suffixes follow `find` convention: `+` means "more than N ago" (older files), `-` means "less than N ago" (newer files).

**MIME filter** — `-t <TYPE>` / `--type <TYPE>`

```bash
# Exact match
mlocate -t text/plain myfile

# Glob match
mlocate -t "image/*" screenshot
mlocate -t "text/*" notes
```

### Examples

```bash
# Basic search
mlocate budget.xlsx

# Case-insensitive basename search
mlocate -i -b readme

# Regex search for Rust files (trigram-accelerated via literal extraction)
mlocate -r '\.rs$'

# Case-insensitive regex
mlocate -i -r readme

# Search with all filters
mlocate --size 1MB+ --modified 30d- -t "image/*" photo

# Count results
mlocate -c "*.log"

# JSON output (streamed, no buffering)
mlocate --json config

# NUL-terminated for xargs (streamed)
mlocate -0 "*.tmp" | xargs -0 rm

# Filter-only search (no pattern, just filters)
mlocate --size 10MB+ --modified 7d-

# Database statistics (includes format_version and trigram distribution)
mlocate --statistics
```

### GNU Compatibility

Use `--gnu` to enable GNU `locate` flag compatibility. Unsupported flags produce warnings instead of errors:

```bash
mlocate --gnu -L -i myfile   # -L is accepted with a warning
```

Supported GNU flags (silently accepted in `--gnu` mode):
`-L`/`--follow`, `-A`/`--all`, `-w`/`--wholename`, `-P`/`--nofollow`, `-H`, `--max-database-age`, `--regextype`, `--require-visibility`

### Shell Completions

```bash
eval "$(mlocate --generate-completions bash)"
eval "$(mlocate --generate-completions zsh)"
mlocate --generate-completions fish > ~/.config/fish/completions/mlocate.fish
```

---

## Database Management

The database is stored at:
- **macOS**: `~/Library/Caches/mlocate/mlocate.db`
- **Linux**: `~/.cache/mlocate/mlocate.db`

Override with `--database` on either command.

> **Note:** The database is memory-mapped for performance. If the file is truncated while mlocate is running, a SIGBUS handler provides a clean error message. A CRC32 footer checksum is validated on open to detect corruption or truncation.

```bash
# Inspect database stats and schema (includes format version and trigram distribution)
mlocate --statistics

# Force full reindex (ignores existing directory table)
mupdatedb --force

# Atomic index update (default)
mupdatedb  # writes to .tmp, CRC32-checksums, then atomically renames
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Matches found |
| `1` | No matches found or empty index. Both conditions are indistinguishable. |
| `2` | Error (missing DB, checksum mismatch, bad filter syntax, etc.) |

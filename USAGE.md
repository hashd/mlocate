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
| `--incremental` | Skip subtrees with unchanged directory mtime |
| `--force` | Full reindex, ignoring existing database |
| `--dry-run` | Report what would be indexed without writing to database |
| `--parallel <N>` | Number of extractor worker threads (default: 4) |
| `--database <PATH>` | Override database file path |
| `--quiet` | Suppress progress output |
| `--install-cron` | Install auto-index scheduling (LaunchAgent on macOS, systemd user timer on Linux) |

### Examples

```bash
# Index home directory with defaults
mupdatedb

# Index specific paths
mupdatedb --localpaths /home/projects --prunepaths /home/projects/node_modules

# Quick incremental update
mupdatedb --incremental

# Preview what would be indexed
mupdatedb --dry-run --localpaths /tmp/testdir
```

---

## mlocate — Search

```bash
mlocate [OPTIONS] [PATTERNS...]
```

### Options

| Flag | Description |
|------|-------------|
| `-i`, `--ignore-case` | Case-insensitive matching. Case folding is ASCII-only. Non-ASCII characters in paths are matched as-is. |
| `-b`, `--basename` | Match pattern against filename only |
| `-r`, `--regex` | Treat pattern as regex. Note: regex search does a full scan (not trigram-accelerated) and will be slower for large databases. |
| `-e`, `--existing` | Only show files currently on disk |
| `-l`, `--limit <N>` | Cap results to N entries |
| `-c`, `--count` | Print match count instead of results |
| `-0`, `--null` | NUL-terminated output (for `xargs -0`) |
| `--size <FILTER>` | Filter by file size (see below) |
| `--modified <FILTER>` | Filter by modification time (see below) |
| `-t`, `--type <MIME>` | Filter by MIME type |
| `--gnu` | GNU locate compatibility mode |
| `-S`, `--schema` | Print database stats as JSON |
| `--database <PATH>` | Override database file path |
| `--table` | Force table output |
| `--json` | JSON array output |
| `--plain` | Plain path-per-line output |
| `--icons` | Show Nerd Font file type icons. **Requires a Nerd Font installed and configured in your terminal.** |
| `--color <MODE>` | Color: `auto`, `always`, `never` (default: `auto`) |
| `--generate-completions <SHELL>` | Generate shell completions (`bash`, `zsh`, `fish`) |

### Output Format

Output is auto-detected based on stdout:
- **TTY**: Modern table view (Path, Size, Modified) via pager
- **Pipe**: Plain text, one path per line
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

# Regex search for Rust files
mlocate -r '\.rs$'

# Search with all filters
mlocate --size 1MB+ --modified 30d- -t "image/*" photo

# Count results
mlocate -c "*.log"

# JSON output
mlocate --json config

# NUL-terminated for xargs
mlocate -0 "*.tmp" | xargs -0 rm
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

> **Note:** The database is memory-mapped for performance. If the file is truncated while mlocate is running, it will crash with SIGBUS. This is rare but can happen if an external process modifies the database during a search.

```bash
# Inspect database schema and stats
mlocate -S

# Force full reindex
mupdatedb --force

# Atomic index update (default)
mupdatedb  # writes to .tmp then atomically renames
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Matches found |
| `1` | No matches found or empty index. Both conditions are indistinguishable. |
| `2` | Error (missing DB, bad filter syntax, etc.) |

use clap::{Args, Parser, ValueEnum};

#[derive(Parser)]
#[command(name = "mlocate", about = "Modern locate alternative", long_about = None, disable_help_flag = true)]
pub struct SearchCli {
    /// Literal substring match on full path (default). Multiple patterns combined with OR.
    #[arg(required_unless_present_any = ["stats", "help", "version"])]
    pub patterns: Vec<String>,

    /// Case-insensitive matching (full Unicode case folding).
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// Match pattern against filename only (the part after the last '/').
    #[arg(short = 'b', long = "basename")]
    pub basename: bool,

    /// Treat pattern as regex instead of literal substring.
    #[arg(short = 'r', long = "regex")]
    pub regex: bool,

    /// Only print files that currently exist on disk.
    #[arg(short = 'e', long = "existing")]
    pub existing: bool,

    /// Cap results to N entries.
    #[arg(short = 'l', long = "limit")]
    pub limit: Option<usize>,

    /// Print only the count of matching entries.
    #[arg(short = 'c', long = "count")]
    pub count: bool,

    /// NUL-terminated output (for xargs -0).
    #[arg(short = '0', long = "null", conflicts_with_all = ["table", "json", "plain"])]
    pub null: bool,

    /// Filter by file size. Format: <value><unit><suffix>.Examples: '10MB+', '1KB-', '500MB'.
    #[arg(long = "size", verbatim_doc_comment)]
    pub size: Option<String>,

    /// Filter by modification time relative to now. Format: <value><unit><suffix>.
    /// Units: m (minutes), h (hours), d (days), w (weeks). Examples: '2d-', '1w+', '30m'.
    #[arg(long = "modified", verbatim_doc_comment)]
    pub modified: Option<String>,

    /// Filter by MIME type. Exact ('text/plain', 'image/png') or glob ('image/*', 'text/*').
    #[arg(short = 't', long = "type", verbatim_doc_comment)]
    pub mime_type: Option<String>,

    /// GNU locate compatibility mode.
    #[arg(long = "gnu")]
    pub gnu: bool,

    /// Print database statistics as JSON.
    #[arg(short = 'S', long = "statistics", visible_aliases = ["schema", "stats"])]
    pub stats: bool,

    /// Override database file path.
    #[arg(long = "database", verbatim_doc_comment)]
    pub database: Option<String>,

    /// Modern table output with columns.
    #[arg(long = "table", conflicts_with_all = ["json", "plain", "null"])]
    pub table: bool,

    /// Output results as JSON array.
    #[arg(long = "json", conflicts_with_all = ["table", "plain", "null"])]
    pub json: bool,

    /// Output one path per line (GNU locate compatible).
    #[arg(long = "plain", conflicts_with_all = ["table", "json", "null"])]
    pub plain: bool,

    /// Show icons for file types (requires Nerd Font).
    #[arg(long = "icons")]
    pub icons: bool,

    /// Color control: auto (default), always, never.
    #[arg(long = "color", default_value = "auto")]
    pub color: ColorMode,

    /// Verbose output.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Print shell completion script.
    #[arg(long = "generate-completions", value_enum)]
    pub generate_completions: Option<Shell>,

    /// Print help.
    #[arg(short = 'h', long = "help")]
    pub help: bool,

    /// Print version.
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    /// Stubs for GNU compat flags accepted with warning in --gnu mode (hidden from help)
    #[arg(short = 'L', long = "follow", hide = true)]
    pub follow: bool,
    #[arg(short = 'A', long = "all", hide = true)]
    pub all: bool,
    #[arg(short = 'w', long = "wholename", hide = true)]
    pub wholename: bool,
    #[arg(short = 'P', long = "nofollow", hide = true)]
    pub nofollow: bool,
    #[arg(short = 'H', hide = true)]
    pub h_flag: bool,
    #[arg(long = "max-database-age", hide = true)]
    pub max_database_age: Option<String>,
    #[arg(long = "regextype", hide = true)]
    pub regextype: Option<String>,
    #[arg(long = "require-visibility", hide = true)]
    pub require_visibility: bool,
}

#[derive(Args)]
pub struct UpdateArgs {
    /// Root directories to scan. Repeatable.
    #[arg(long = "localpaths")]
    pub localpaths: Vec<String>,

    /// Paths to exclude from scan. Repeatable. Matched against canonical path.
    #[arg(long = "prunepaths")]
    pub prunepaths: Vec<String>,

    /// Skip subtrees with unchanged directory mtime (requires existing index).
    #[arg(long = "incremental")]
    pub incremental: bool,

    /// Perform a full reindex (ignore existing directory table).
    #[arg(long = "force")]
    pub force: bool,

    /// Walk the filesystem and report what would be indexed, but do not create/modify database.
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Number of extractor worker threads.
    #[arg(long = "parallel")]
    pub parallel: Option<usize>,

    /// Override database file path.
    #[arg(long = "database")]
    pub database: Option<String>,

    /// Suppress all progress output.
    #[arg(long = "quiet")]
    pub quiet: bool,

    /// Verbose output.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Skip magic-byte MIME detection for extensionless files (faster, but may lose MIME accuracy).
    #[arg(long = "no-magic-mime")]
    pub no_magic_mime: bool,

    /// Install auto-index scheduling for the current user.
    #[arg(long = "install-cron")]
    pub install_cron: bool,
}

#[derive(Parser)]
#[command(name = "mupdatedb", about = "Update the mlocate database", long_about = None)]
pub struct UpdateCli {
    #[command(flatten)]
    pub args: UpdateArgs,

    /// Print version.
    #[arg(short = 'V', long = "version")]
    pub version: bool,
}

#[derive(Clone, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Clone, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

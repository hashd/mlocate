use clap::CommandFactory;
use clap::Parser;
use mlocate::cli::SearchCli;

fn main() -> anyhow::Result<()> {
    let cli = SearchCli::parse();

    if cli.help {
        SearchCli::command().print_help()?;
        println!();
        return Ok(());
    }

    if cli.schema {
        println!("{{\"status\": \"schema not yet implemented\"}}");
        return Ok(());
    }

    if cli.patterns.is_empty() {
        eprintln!("Error: A search pattern is required. Usage: mlocate [OPTIONS] <pattern>");
        std::process::exit(2);
    }

    println!("Search stub: patterns={:?}", cli.patterns);
    Ok(())
}

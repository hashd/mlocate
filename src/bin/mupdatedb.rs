use clap::Parser;
use mlocate::cli::UpdateCli;

fn main() -> anyhow::Result<()> {
    let cli = UpdateCli::parse();

    if cli.args.install_cron {
        eprintln!("--install-cron not yet implemented");
        return Ok(());
    }

    let _is_dry_run = cli.args.dry_run;

    println!("Index stub: localpaths={:?}", cli.args.localpaths);
    Ok(())
}

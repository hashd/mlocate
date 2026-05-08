use std::io::Write;
use std::process::{Command, Stdio};

pub fn page_output(output: &str) -> anyhow::Result<()> {
    if std::env::var("NOPAGER").is_ok() || !is_terminal::is_terminal(std::io::stdout()) {
        print!("{}", output);
        return Ok(());
    }

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
    let parts = match shlex::split(&pager) {
        Some(p) => p,
        None => {
            print!("{}", output);
            return Ok(());
        }
    };
    if parts.is_empty() {
        print!("{}", output);
        return Ok(());
    }

    let mut child = Command::new(&parts[0])
        .args(&parts[1..])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        if let Err(e) = stdin.write_all(output.as_bytes()) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(e.into());
            }
        }
    }

    let _ = child.wait();
    Ok(())
}

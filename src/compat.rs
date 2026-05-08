use crate::cli::SearchCli;

pub fn warn_gnu_stubs(cli: &SearchCli) {
    let stubs: Vec<(&str, bool)> = vec![
        ("-L/--follow", cli.follow && !cli.gnu),
        ("-A/--all", cli.all),
        ("-w/--wholename", cli.wholename),
        ("-P/--nofollow", cli.nofollow),
        ("-H", cli.h_flag),
        ("--max-database-age", cli.max_database_age.is_some()),
        ("--regextype", cli.regextype.is_some()),
        ("--require-visibility", cli.require_visibility),
    ];

    for (flag, present) in &stubs {
        if *present {
            if cli.gnu {
                eprintln!("Warning: {} is not supported in mlocate and will be ignored.", flag);
            } else {
                eprintln!("Error: {} is not supported in mlocate. Use --gnu for GNU locate compatibility mode.", flag);
                std::process::exit(2);
            }
        }
    }
}

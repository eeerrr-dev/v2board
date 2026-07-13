#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Serve,
    Migrate,
    ResetAdminPassword { email: String },
    Help,
}

pub(crate) fn parse() -> anyhow::Result<Command> {
    parse_args(std::env::args().skip(1))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Command> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(Command::Serve),
        [flag] if matches!(flag.as_str(), "--help" | "-h") => Ok(Command::Help),
        [command] if command == "migrate" => Ok(Command::Migrate),
        [command, email] if command == "reset-admin-password" => Ok(Command::ResetAdminPassword {
            email: email.clone(),
        }),
        _ => anyhow::bail!(
            "invalid command; run v2board-api --help for the supported command grammar"
        ),
    }
}

pub(crate) fn print_help() {
    println!(
        "v2board-api\n\nCommands:\n  migrate\n      Apply native PostgreSQL migrations\n\n  reset-admin-password <email>\n      Read the new password from a systemd credential or V2BOARD_NEW_PASSWORD_FILE\n\nMySQL import checks are intentionally absent from this runtime binary. Use the disposable\nv2board-lifecycle binary for validate/inspect operations."
    );
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_args};

    #[test]
    fn accepts_only_runtime_administration_commands() {
        assert_eq!(parse_args(std::iter::empty()).unwrap(), Command::Serve);
        assert_eq!(
            parse_args(["migrate"].map(str::to_owned)).unwrap(),
            Command::Migrate
        );
        assert_eq!(
            parse_args(["reset-admin-password", "admin@example.com"].map(str::to_owned)).unwrap(),
            Command::ResetAdminPassword {
                email: "admin@example.com".to_string()
            }
        );
    }

    #[test]
    fn rejects_import_commands() {
        assert!(
            parse_args(
                ["provision", "inspect", "--manifest", "mysql-import.json"].map(str::to_owned)
            )
            .is_err()
        );
        assert!(
            parse_args(["inspect", "--manifest", "mysql-import.json"].map(str::to_owned)).is_err()
        );
    }
}

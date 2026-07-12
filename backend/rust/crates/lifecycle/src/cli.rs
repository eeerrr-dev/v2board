use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Validate { manifest: PathBuf },
    Inspect { manifest: PathBuf },
    Plan { manifest: PathBuf },
    Help,
}

pub(crate) fn parse() -> anyhow::Result<Command> {
    parse_args(std::env::args().skip(1))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Command> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if matches!(flag.as_str(), "--help" | "-h") => Ok(Command::Help),
        [action, flag, manifest]
            if flag == "--manifest"
                && matches!(action.as_str(), "validate" | "inspect" | "plan") =>
        {
            let manifest = PathBuf::from(manifest);
            match action.as_str() {
                "validate" => Ok(Command::Validate { manifest }),
                "inspect" => Ok(Command::Inspect { manifest }),
                "plan" => Ok(Command::Plan { manifest }),
                _ => unreachable!("guard accepts every lifecycle action"),
            }
        }
        _ => anyhow::bail!(
            "invalid command; run v2board-lifecycle --help for the supported command grammar"
        ),
    }
}

pub(crate) fn print_help() {
    println!(
        "v2board-lifecycle\n\nDisposable lifecycle commands:\n  validate --manifest <path>\n      Strictly validate a complete lifecycle JSON without connecting\n\n  inspect --manifest <path>\n      Run the online read-only compatibility inspection; compatible never means ready to migrate\n\n  plan --manifest <path>\n      Expose the fenced final read-only recheck for development review; a future one-shot legacy\n      apply runs it internally without another human pause\n\nThis binary is deliberately separate from v2board-api and v2board-workers. It is the only binary\nthat can contain the legacy MySQL source adapter. There is no apply command, and both inspection\nmodes keep apply_available=false."
    );
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Command, parse_args};

    #[test]
    fn accepts_only_the_three_manifest_commands() {
        assert_eq!(
            parse_args(["validate", "--manifest", "/secure/operation.json"].map(str::to_owned))
                .unwrap(),
            Command::Validate {
                manifest: PathBuf::from("/secure/operation.json")
            }
        );
        assert_eq!(
            parse_args(["inspect", "--manifest", "operation.json"].map(str::to_owned)).unwrap(),
            Command::Inspect {
                manifest: PathBuf::from("operation.json")
            }
        );
        assert_eq!(
            parse_args(["plan", "--manifest", "operation.json"].map(str::to_owned)).unwrap(),
            Command::Plan {
                manifest: PathBuf::from("operation.json")
            }
        );
    }

    #[test]
    fn rejects_runtime_and_legacy_api_grammars() {
        assert!(parse_args(std::iter::empty()).is_err());
        assert!(parse_args(["serve"].map(str::to_owned)).is_err());
        assert!(
            parse_args(["provision", "inspect", "--manifest", "operation.json"].map(str::to_owned))
                .is_err()
        );
        assert!(parse_args(["apply", "--manifest", "operation.json"].map(str::to_owned)).is_err());
    }
}

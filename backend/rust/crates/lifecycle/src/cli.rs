use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Validate {
        manifest: PathBuf,
    },
    Inspect {
        manifest: PathBuf,
    },
    Execute {
        manifest: PathBuf,
    },
    InspectReleaseArchive {
        archive: PathBuf,
        release_id: String,
        sha256: String,
    },
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
                && matches!(action.as_str(), "validate" | "inspect" | "execute") =>
        {
            let manifest = PathBuf::from(manifest);
            match action.as_str() {
                "validate" => Ok(Command::Validate { manifest }),
                "inspect" => Ok(Command::Inspect { manifest }),
                "execute" => Ok(Command::Execute { manifest }),
                _ => unreachable!("guard covers every accepted action"),
            }
        }
        [
            action,
            archive_flag,
            archive,
            release_id_flag,
            release_id,
            sha256_flag,
            sha256,
        ] if action == "inspect-release-archive"
            && archive_flag == "--archive"
            && release_id_flag == "--release-id"
            && sha256_flag == "--sha256" =>
        {
            Ok(Command::InspectReleaseArchive {
                archive: PathBuf::from(archive),
                release_id: release_id.clone(),
                sha256: sha256.clone(),
            })
        }
        _ => anyhow::bail!(
            "invalid command; run v2board-lifecycle --help for the supported command grammar"
        ),
    }
}

pub(crate) fn print_help() {
    println!(
        "v2board-lifecycle\n\nMySQL import:\n  validate --manifest <path>\n      Validate the single pre-release schema-v1 import manifest without connecting anywhere\n\n  inspect --manifest <path>\n      Safely read and hash the manifest-bound backup dump without contacting old MySQL,\n      old Redis, Stripe, or any target\n\n  execute --manifest <path>\n      Read the stopped legacy MySQL through a local read-only snapshot, convert typed rows\n      into one absent PostgreSQL/ClickHouse target, initialize empty Redis, and emit configs\n\nIndependent deployment check:\n  inspect-release-archive --archive <absolute-path> --release-id <id> --sha256 <sha256>\n      Validate a native release archive without mutating the filesystem"
    );
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Command, parse_args};

    #[test]
    fn accepts_import_checks() {
        for (action, expected) in [
            (
                "validate",
                Command::Validate {
                    manifest: PathBuf::from("/secure/mysql-import.json"),
                },
            ),
            (
                "inspect",
                Command::Inspect {
                    manifest: PathBuf::from("/secure/mysql-import.json"),
                },
            ),
            (
                "execute",
                Command::Execute {
                    manifest: PathBuf::from("/secure/mysql-import.json"),
                },
            ),
        ] {
            assert_eq!(
                parse_args([action, "--manifest", "/secure/mysql-import.json"].map(str::to_owned))
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            parse_args(
                [
                    "inspect-release-archive",
                    "--archive",
                    "/secure/native-release.tar.gz",
                    "--release-id",
                    "release-a",
                    "--sha256",
                    &"a".repeat(64),
                ]
                .map(str::to_owned),
            )
            .unwrap(),
            Command::InspectReleaseArchive {
                archive: PathBuf::from("/secure/native-release.tar.gz"),
                release_id: "release-a".to_string(),
                sha256: "a".repeat(64),
            }
        );
    }

    #[test]
    fn rejects_malformed_commands() {
        assert!(parse_args(std::iter::empty()).is_err());
        assert!(
            parse_args(["inspect", "--wrong-flag", "mysql-import.json"].map(str::to_owned))
                .is_err()
        );
    }
}

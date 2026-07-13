use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Validate {
        manifest: PathBuf,
    },
    Inspect {
        manifest: PathBuf,
    },
    InspectReleaseArchive {
        archive: PathBuf,
        release_id: String,
        sha256: String,
    },
    Apply {
        manifest: PathBuf,
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
                && matches!(action.as_str(), "validate" | "inspect" | "apply") =>
        {
            let manifest = PathBuf::from(manifest);
            match action.as_str() {
                "validate" => Ok(Command::Validate { manifest }),
                "inspect" => Ok(Command::Inspect { manifest }),
                "apply" => Ok(Command::Apply { manifest }),
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
        "v2board-lifecycle\n\nArchive-first cold-import commands:\n  validate --manifest <path>\n      Strictly validate the unique schema-v5 loss policy without connecting anywhere\n\n  inspect --manifest <path>\n      Read and hash the immutable encrypted MySQL dump, age identity, and native release\n      without contacting legacy MySQL, legacy Redis, Stripe, or mutating a target\n\n  inspect-release-archive --archive <absolute-path> --release-id <id> --sha256 <sha256>\n      Run the mutation-free native release archive contract inspector\n\n  apply --manifest <path>\n      Reserved cold-import entry; currently fails closed before writes\n\nThe operator stops the old site and creates the immutable encrypted MySQL dump before this tool is\nused. A failed, unactivated import is wiped and restarted from the same dump. Production apply is\ncurrently fail-closed until the importer and operation-owned cleanup integration gate pass."
    );
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Command, parse_args};

    #[test]
    fn accepts_the_small_archive_first_grammar() {
        for (action, expected) in [
            (
                "validate",
                Command::Validate {
                    manifest: PathBuf::from("/secure/operation.json"),
                },
            ),
            (
                "inspect",
                Command::Inspect {
                    manifest: PathBuf::from("/secure/operation.json"),
                },
            ),
            (
                "apply",
                Command::Apply {
                    manifest: PathBuf::from("/secure/operation.json"),
                },
            ),
        ] {
            assert_eq!(
                parse_args([action, "--manifest", "/secure/operation.json"].map(str::to_owned))
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
    fn rejects_authorize_resume_and_old_apply_grammar() {
        assert!(parse_args(std::iter::empty()).is_err());
        assert!(
            parse_args(
                [
                    "authorize",
                    "--manifest",
                    "operation.json",
                    "--inspect-review-sha256",
                    &"a".repeat(64),
                    "--output",
                    "/secure/authorization.json",
                ]
                .map(str::to_owned)
            )
            .is_err()
        );
        assert!(
            parse_args(
                [
                    "resume",
                    "--manifest",
                    "operation.json",
                    "--authorization",
                    "/secure/authorization.json",
                ]
                .map(str::to_owned)
            )
            .is_err()
        );
        assert!(
            parse_args(
                [
                    "apply",
                    "--manifest",
                    "operation.json",
                    "--authorization",
                    "/secure/authorization.json",
                ]
                .map(str::to_owned)
            )
            .is_err()
        );
    }
}

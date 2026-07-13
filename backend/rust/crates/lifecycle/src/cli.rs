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
    Authorize {
        manifest: PathBuf,
        inspect_review_sha256: String,
        output: PathBuf,
    },
    Apply {
        manifest: PathBuf,
        authorization: PathBuf,
    },
    Resume {
        manifest: PathBuf,
        authorization: PathBuf,
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
            if flag == "--manifest" && matches!(action.as_str(), "validate" | "inspect") =>
        {
            let manifest = PathBuf::from(manifest);
            match action.as_str() {
                "validate" => Ok(Command::Validate { manifest }),
                "inspect" => Ok(Command::Inspect { manifest }),
                _ => unreachable!("guard accepts every lifecycle action"),
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
        [
            action,
            manifest_flag,
            manifest,
            report_flag,
            review_sha256,
            output_flag,
            output,
        ] if action == "authorize"
            && manifest_flag == "--manifest"
            && report_flag == "--inspect-review-sha256"
            && output_flag == "--output" =>
        {
            Ok(Command::Authorize {
                manifest: PathBuf::from(manifest),
                inspect_review_sha256: review_sha256.clone(),
                output: PathBuf::from(output),
            })
        }
        [
            action,
            manifest_flag,
            manifest,
            authorization_flag,
            authorization,
        ] if matches!(action.as_str(), "apply" | "resume")
            && manifest_flag == "--manifest"
            && authorization_flag == "--authorization" =>
        {
            let manifest = PathBuf::from(manifest);
            let authorization = PathBuf::from(authorization);
            if action == "apply" {
                Ok(Command::Apply {
                    manifest,
                    authorization,
                })
            } else {
                Ok(Command::Resume {
                    manifest,
                    authorization,
                })
            }
        }
        _ => anyhow::bail!(
            "invalid command; run v2board-lifecycle --help for the supported command grammar"
        ),
    }
}

pub(crate) fn print_help() {
    println!(
        "v2board-lifecycle\n\nDisposable lifecycle commands:\n  validate --manifest <path>\n      Strictly validate a complete lifecycle JSON without connecting\n\n  inspect --manifest <path>\n      Run the online read-only compatibility inspection; compatible never means ready to migrate\n\n  inspect-release-archive --archive <absolute-path> --release-id <id> --sha256 <sha256>\n      Run the mutation-free native archive contract inspector; this does not authorize migration\n\n  authorize --manifest <path> --inspect-review-sha256 <sha256> --output <absolute-path>\n      Re-run the reviewed online inspection and require the stable identity/schema/policy binding\n      before the operator types the exact operation_id\n\n  apply --manifest <path> --authorization <absolute-path>\n      Start the one irreversible operation bound to the reviewed inspection and authorization\n\n  resume --manifest <path> --authorization <absolute-path>\n      Forward-recover that same durable operation; this is not a second cutover decision\n\nThis binary is deliberately separate from v2board-api and v2board-workers. It is the only binary\nthat can contain the legacy MySQL source adapter. Apply and resume fail closed while the production\nfault-injection gate remains disabled."
    );
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Command, parse_args};

    #[test]
    fn accepts_manifest_and_authorization_commands() {
        assert_eq!(
            parse_args(["validate", "--manifest", "/secure/operation.json"].map(str::to_owned))
                .unwrap(),
            Command::Validate {
                manifest: PathBuf::from("/secure/operation.json")
            }
        );
        assert_eq!(
            parse_args(
                [
                    "apply",
                    "--manifest",
                    "operation.json",
                    "--authorization",
                    "/secure/authorization.json",
                ]
                .map(str::to_owned),
            )
            .unwrap(),
            Command::Apply {
                manifest: PathBuf::from("operation.json"),
                authorization: PathBuf::from("/secure/authorization.json"),
            }
        );
        assert_eq!(
            parse_args(
                [
                    "resume",
                    "--manifest",
                    "operation.json",
                    "--authorization",
                    "/secure/authorization.json",
                ]
                .map(str::to_owned),
            )
            .unwrap(),
            Command::Resume {
                manifest: PathBuf::from("operation.json"),
                authorization: PathBuf::from("/secure/authorization.json"),
            }
        );
        assert_eq!(
            parse_args(["inspect", "--manifest", "operation.json"].map(str::to_owned)).unwrap(),
            Command::Inspect {
                manifest: PathBuf::from("operation.json")
            }
        );
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
        assert_eq!(
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
                .map(str::to_owned),
            )
            .unwrap(),
            Command::Authorize {
                manifest: PathBuf::from("operation.json"),
                inspect_review_sha256: "a".repeat(64),
                output: PathBuf::from("/secure/authorization.json"),
            }
        );
    }

    #[test]
    fn rejects_runtime_and_legacy_api_grammars() {
        assert!(parse_args(std::iter::empty()).is_err());
        assert!(parse_args(["serve"].map(str::to_owned)).is_err());
        assert!(parse_args(["plan", "--manifest", "operation.json"].map(str::to_owned)).is_err());
        assert!(
            parse_args(["provision", "inspect", "--manifest", "operation.json"].map(str::to_owned))
                .is_err()
        );
        assert!(parse_args(["apply", "--manifest", "operation.json"].map(str::to_owned)).is_err());
        assert!(
            parse_args(
                [
                    "inspect-release-archive",
                    "--release-id",
                    "release-a",
                    "--archive",
                    "/secure/native-release.tar.gz",
                    "--sha256",
                    &"a".repeat(64),
                ]
                .map(str::to_owned),
            )
            .is_err()
        );
        assert!(
            parse_args(
                [
                    "resume",
                    "--authorization",
                    "/secure/authorization.json",
                    "--manifest",
                    "operation.json",
                ]
                .map(str::to_owned),
            )
            .is_err()
        );
    }
}

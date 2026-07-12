use std::path::PathBuf;

pub(crate) enum Command {
    Serve,
    Migrate,
    ResetAdminPassword { email: String },
    ProvisionValidate { manifest: PathBuf },
    ProvisionInspect { manifest: PathBuf },
    ProvisionPlan { manifest: PathBuf },
    Help,
}

pub(crate) fn parse() -> anyhow::Result<Command> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(Command::Serve),
        [flag] if matches!(flag.as_str(), "--help" | "-h") => Ok(Command::Help),
        [command] if command == "migrate" => Ok(Command::Migrate),
        [command, email] if command == "reset-admin-password" => Ok(Command::ResetAdminPassword {
            email: email.clone(),
        }),
        [provision, action, flag, manifest]
            if provision == "provision"
                && flag == "--manifest"
                && matches!(action.as_str(), "validate" | "inspect" | "plan") =>
        {
            let manifest = PathBuf::from(manifest);
            match action.as_str() {
                "validate" => Ok(Command::ProvisionValidate { manifest }),
                "inspect" => Ok(Command::ProvisionInspect { manifest }),
                "plan" => Ok(Command::ProvisionPlan { manifest }),
                _ => unreachable!("guard accepts every provision action"),
            }
        }
        _ => anyhow::bail!(
            "invalid command; run v2board-api --help for the supported command grammar"
        ),
    }
}

pub(crate) fn print_help() {
    println!(
        "v2board-api\n\nCommands:\n  migrate\n      Apply native database migrations (never a legacy adoption command)\n\n  reset-admin-password <email>\n      Read the new password from V2BOARD_NEW_PASSWORD\n\n  provision validate --manifest <path>\n      Strictly validate a versioned, complete file-only lifecycle JSON without connecting\n\n  provision inspect --manifest <path>\n      Run the online read-only compatibility inspection; compatible never means ready to migrate\n\n  provision plan --manifest <path>\n      After entering a fenced maintenance window, run the final read-only plan; a successful\n      result is ready for explicit confirmation, not permission to write or cut over\n\nTwo confirmations are required by the lifecycle: first before entering maintenance, then against\nthe exact final operation ID and report SHA before any future apply. There is no provision apply\ncommand, and both inspection modes keep apply_available=false."
    );
}

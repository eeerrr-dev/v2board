mod cli;

fn main() -> anyhow::Result<()> {
    match cli::parse()? {
        cli::Command::Help => cli::print_help(),
        cli::Command::InspectReleaseArchive {
            archive,
            release_id,
            sha256,
        } => {
            require_absolute_normalized(&archive, "--archive")?;
            let inspection =
                v2board_provision::release_archive::inspect_native_release_archive_read_only(
                    &archive,
                    &release_id,
                    &sha256,
                )?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
        }
        cli::Command::Validate { manifest } => {
            let spec = v2board_provision::load_mysql_import_spec(manifest)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "manifest_valid": true,
                    "schema_version": spec.schema_version,
                    "reference_commit": v2board_provision::MYSQL_SOURCE_REFERENCE_COMMIT,
                    "manifest_sha256": spec.manifest_sha256(),
                    "old_mysql_contacted": false,
                    "old_redis_contacted": false,
                    "stripe_provider_contacted": false,
                    "staging_mysql_contacted": false,
                    "target_mutated": false,
                    "secrets_redacted": true
                }))?
            );
        }
        cli::Command::Inspect { manifest } => {
            let spec = v2board_provision::load_mysql_import_spec(manifest)?;
            let inspection = v2board_provision::inspect_mysql_import(&spec)?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
        }
    }
    Ok(())
}

fn require_absolute_normalized(path: &std::path::Path, flag: &str) -> anyhow::Result<()> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir
                    | std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        anyhow::bail!("{flag} must be an absolute normalized path");
    }
    Ok(())
}

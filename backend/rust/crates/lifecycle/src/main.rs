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
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let capability = v2board_provision::production_cold_import_capability_for_spec(&spec);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "manifest_valid": true,
                    "schema_version": spec.schema_version,
                    "operation_id": spec.operation_id,
                    "kind": spec.kind,
                    "reference_commit": v2board_provision::LEGACY_REFERENCE_COMMIT,
                    "manifest_binding_hmac_sha256": spec.manifest_binding_hmac_sha256(),
                    "legacy_mysql_contacted": false,
                    "legacy_redis_contacted": false,
                    "stripe_provider_contacted": false,
                    "target_mutated": false,
                    "apply_available": capability.is_available(),
                    "apply_blocker": capability.blocker().map(|blocker| blocker.report_message()),
                    "secrets_redacted": true
                }))?
            );
        }
        cli::Command::Inspect { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let inspection = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::ArchiveReadOnly,
            )?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
            if !inspection.passed() {
                anyhow::bail!("cold-import archive inspection is blocked; see the JSON report");
            }
        }
        cli::Command::Apply { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let capability = v2board_provision::production_cold_import_capability_for_spec(&spec);
            if let Some(blocker) = capability.blocker() {
                anyhow::bail!(blocker.report_message());
            }
            anyhow::bail!(
                "production cold-import executor is not linked; opening the typed capability without the integration gate is invalid"
            );
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

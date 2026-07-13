mod cli;

use std::{
    io::{self, Write},
    time::{SystemTime, UNIX_EPOCH},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match cli::parse()? {
        cli::Command::Help => cli::print_help(),
        cli::Command::Validate { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let apply_capability = v2board_provision::production_legacy_apply::production_legacy_apply_capability_for_spec(&spec);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "manifest_valid": true,
                    "migration_authorized": false,
                    "apply_available": apply_capability.is_available(),
                    "schema_version": spec.schema_version,
                    "operation_id": spec.operation_id,
                    "reference_commit": v2board_provision::LEGACY_REFERENCE_COMMIT,
                    "manifest_binding_hmac_sha256": spec.manifest_binding_hmac_sha256(),
                    "secrets_redacted": true
                }))?
            );
        }
        cli::Command::Inspect { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let inspection = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::Online,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
            if !inspection.passed() {
                anyhow::bail!(
                    "online lifecycle compatibility inspection is blocked; see the JSON report"
                );
            }
        }
        cli::Command::Authorize {
            manifest,
            inspect_review_sha256,
            output,
        } => {
            if inspect_review_sha256.len() != 64
                || !inspect_review_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                anyhow::bail!("--inspect-review-sha256 must be 64 lowercase hexadecimal bytes");
            }
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let execution = spec.legacy_apply_execution().ok_or_else(|| {
                anyhow::anyhow!(
                    "authorization requires a schema_version 4 legacy manifest with execution inputs"
                )
            })?;
            if output != execution.journal.authorization_path {
                anyhow::bail!(
                    "--output must exactly match execution.journal.authorization_path in the HMAC-bound manifest"
                );
            }
            let inspection = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::Online,
            )
            .await?;
            if inspection.review_binding_sha256 != inspect_review_sha256 {
                anyhow::bail!(
                    "the current source/target identity, schema, or policy no longer matches the reviewed inspection binding"
                );
            }
            eprintln!(
                "Reviewed binding {} still matches and the current dynamic preflight is ready (snapshot {}). Type the exact operation_id {} to authorize the irreversible one-shot maintenance operation:",
                inspection.review_binding_sha256, inspection.report_sha256, spec.operation_id
            );
            io::stderr().flush()?;
            let mut confirmation = String::new();
            io::stdin().read_line(&mut confirmation)?;
            let now_unix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let now_unix = i64::try_from(now_unix)
                .map_err(|_| anyhow::anyhow!("system clock exceeds the supported Unix range"))?;
            let authorization = v2board_provision::ApplyAuthorization::issue(
                &spec,
                &inspection,
                confirmation.trim_end_matches(['\r', '\n']),
                now_unix,
            )?;
            authorization.write_new(&output)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "authorization_written": true,
                    "operation_id": authorization.operation_id,
                    "inspect_review_sha256": authorization.inspect_review_sha256,
                    "authorized_snapshot_report_sha256": authorization.authorized_snapshot_report_sha256,
                    "expires_at_unix": authorization.expires_at_unix,
                    "output": output,
                    "secrets_redacted": true
                }))?
            );
        }
        cli::Command::Apply {
            manifest,
            authorization,
        } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            require_exact_authorization_path(&spec, &authorization)?;
            let (authorization, authorization_file_sha256) =
                v2board_provision::ApplyAuthorization::load_with_file_sha256(&authorization)?;
            authorization.verify_resume_binding(&spec)?;
            let now_unix = unix_now()?;
            let result = v2board_provision::production_legacy_apply::start_production_legacy_apply(
                &spec,
                &authorization,
                &authorization_file_sha256,
                now_unix,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        cli::Command::Resume {
            manifest,
            authorization,
        } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            require_exact_authorization_path(&spec, &authorization)?;
            let (authorization, authorization_file_sha256) =
                v2board_provision::ApplyAuthorization::load_with_file_sha256(&authorization)?;
            authorization.verify_resume_binding(&spec)?;
            let result =
                v2board_provision::production_legacy_apply::resume_production_legacy_apply(
                    &spec,
                    &authorization,
                    &authorization_file_sha256,
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

fn require_exact_authorization_path(
    spec: &v2board_provision::ProvisionSpec,
    supplied: &std::path::Path,
) -> anyhow::Result<()> {
    let expected = &spec
        .legacy_apply_execution()
        .ok_or_else(|| anyhow::anyhow!("schema-v4 legacy execution inputs are required"))?
        .journal
        .authorization_path;
    if supplied != expected {
        anyhow::bail!(
            "--authorization must exactly match execution.journal.authorization_path in the HMAC-bound manifest"
        );
    }
    Ok(())
}

fn unix_now() -> anyhow::Result<i64> {
    let value = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(value).map_err(|_| anyhow::anyhow!("system clock exceeds the supported range"))
}

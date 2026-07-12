mod cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match cli::parse()? {
        cli::Command::Help => cli::print_help(),
        cli::Command::Validate { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "manifest_valid": true,
                    "migration_authorized": false,
                    "apply_available": false,
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
        cli::Command::Plan { manifest } => {
            let spec = v2board_provision::load_provision_spec(manifest)?;
            let plan = v2board_provision::build_inspection(
                &spec,
                v2board_provision::InspectionMode::FencedFinal,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
            if !plan.passed() {
                anyhow::bail!("fenced final lifecycle plan is blocked; see the JSON report");
            }
        }
    }
    Ok(())
}

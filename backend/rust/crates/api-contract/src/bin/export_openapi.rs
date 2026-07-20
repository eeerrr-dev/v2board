use std::{env, fs, path::PathBuf};

use utoipa::OpenApi as _;
use v2board_api_contract::InternalApiDoc;

fn main() -> anyhow::Result<()> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let (check, output) = match arguments.as_slice() {
        [output] => (false, PathBuf::from(output)),
        [flag, output] if flag == "--check" => (true, PathBuf::from(output)),
        _ => anyhow::bail!("usage: v2board-export-openapi [--check] <internal-api.openapi.json>"),
    };
    let document = InternalApiDoc::openapi();
    let rendered = serde_json::to_string_pretty(&document)? + "\n";
    if check {
        let checked_in = fs::read_to_string(&output)?;
        anyhow::ensure!(
            checked_in == rendered,
            "{} is stale; run `make api-contract-generate`",
            output.display()
        );
    } else {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output, rendered)?;
    }
    Ok(())
}

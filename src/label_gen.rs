use minijinja::{context, Environment};
use tempfile::TempDir;
use tokio::process::Command;

use crate::errors::{AppError, AppResult};

const LABEL_TEMPLATE: &str = include_str!("../templates/labels.tex.j2");

/// Render a LaTeX label sheet for `barcodes` and compile it to PDF bytes via
/// lualatex (required for the barracuda Code128 package).
pub async fn generate_label_pdf(barcodes: &[String], description: &str) -> AppResult<Vec<u8>> {
    let tmp = TempDir::new().map_err(|e| AppError::Internal(format!("tmpdir: {e}")))?;

    // Render template.
    let mut env = Environment::new();
    env.add_template("labels", LABEL_TEMPLATE)
        .map_err(|e| AppError::Internal(format!("template load: {e}")))?;
    let rendered = env
        .get_template("labels")
        .and_then(|t| t.render(context!(barcodes, description)))
        .map_err(|e| AppError::Internal(format!("template render: {e}")))?;

    // Write .tex source.
    let tex = tmp.path().join("labels.tex");
    std::fs::write(&tex, rendered).map_err(|e| AppError::Internal(format!("write tex: {e}")))?;

    // lualatex → PDF directly (barracuda requires LuaTeX)
    run(
        "lualatex",
        &[
            "-interaction=nonstopmode",
            "-halt-on-error",
            &format!("-output-directory={}", tmp.path().display()),
            &tex.to_string_lossy(),
        ],
    )
    .await?;

    tokio::fs::read(tmp.path().join("labels.pdf"))
        .await
        .map_err(|e| AppError::Internal(format!("read pdf: {e}")))
}

async fn run(program: &str, args: &[&str]) -> AppResult<()> {
    let out = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("{program} exec: {e}")))?;

    if !out.status.success() {
        let log = String::from_utf8_lossy(&out.stdout);
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::Internal(format!(
            "{program} failed (exit {}):\n{log}{err}",
            out.status
        )));
    }
    Ok(())
}

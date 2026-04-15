use std::str::FromStr;

use minijinja::{context, Environment};
use tempfile::TempDir;
use tokio::process::Command;

use crate::errors::{AppError, AppResult};

const TPL_30UP: &str = include_str!("../templates/labels-30up.tex.j2");
const TPL_80UP: &str = include_str!("../templates/labels-80up.tex.j2");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStock {
    /// 3×10 on US letter — Avery 5160 compatible. 30 labels/sheet.
    ThirtyUp,
    /// 4×20 on US letter — OnlineLabels OL25WX. 80 labels/sheet.
    EightyUp,
}

impl LabelStock {
    pub fn template(&self) -> &'static str {
        match self {
            LabelStock::ThirtyUp => TPL_30UP,
            LabelStock::EightyUp => TPL_80UP,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LabelStock::ThirtyUp => "30-up",
            LabelStock::EightyUp => "80-up",
        }
    }
}

impl FromStr for LabelStock {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "30-up" => Ok(LabelStock::ThirtyUp),
            "80-up" => Ok(LabelStock::EightyUp),
            other => Err(AppError::BadRequest(format!(
                "unknown label stock \"{other}\" (expected \"30-up\" or \"80-up\")"
            ))),
        }
    }
}

/// Render a LaTeX label sheet for `barcodes` and compile it to PDF bytes via
/// lualatex (required for the barracuda Code128 package).
pub async fn generate_label_pdf(barcodes: &[String], description: &str, stock: LabelStock) -> AppResult<Vec<u8>> {
    let tmp = TempDir::new().map_err(|e| AppError::Internal(format!("tmpdir: {e}")))?;

    // Render template.
    let mut env = Environment::new();
    env.add_template("labels", stock.template())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stock_strings() {
        assert_eq!(LabelStock::from_str("30-up").unwrap(), LabelStock::ThirtyUp);
        assert_eq!(LabelStock::from_str("80-up").unwrap(), LabelStock::EightyUp);
        assert!(LabelStock::from_str("bad").is_err());
    }

    #[test]
    fn stock_selects_distinct_templates() {
        assert!(LabelStock::ThirtyUp.template().contains("\\LabelCols=3"));
        assert!(LabelStock::EightyUp.template().contains("\\LabelCols=4"));
    }
}

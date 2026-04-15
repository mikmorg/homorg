//! v1 enrichment provider: shells out to the `claude` CLI in non-interactive
//! print mode with a JSON schema enforcing output structure.
//!
//! **Why shelling out?** The user's Claude subscription is tied to the CLI's
//! OAuth credentials (in `~/.claude/`). Using the CLI inherits that auth
//! transparently, with no API key management. If `ANTHROPIC_API_KEY` is set,
//! the CLI switches to API billing automatically — same binary, same flags.
//!
//! **Deployment.** The daemon must run on a host where `claude` is installed
//! and logged in. Docker deployment is deferred to a future `ClaudeApiProvider`
//! that uses the Messages HTTP API.
//!
//! **Cache strategy.** The prompt prefix (instructions + allowed tags +
//! allowed categories) is kept byte-identical across calls so Anthropic's
//! prompt cache discounts subsequent runs ~90%. Per-item context goes last.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::models::enrichment::{
    EnrichmentError, EnrichmentInput, EnrichmentOutput, PresetHint,
};

use super::provider::EnrichmentProvider;

/// Runtime config for the CLI provider. The daemon builds this from its
/// env-var config in Phase 3.
#[derive(Debug, Clone)]
pub struct ClaudeCliConfig {
    /// Path to the `claude` binary. Usually `/home/<user>/.local/bin/claude`
    /// or wherever `which claude` points. If `None`, uses `"claude"` (PATH).
    pub cli_path: PathBuf,
    /// Model name passed to `--model`. `claude-opus-4-6` by default.
    pub model: String,
    /// Per-call spend cap passed to `--max-budget-usd`. Zero disables the flag.
    pub per_call_budget_usd: f64,
    /// Overall timeout for a single invocation.
    pub timeout: Duration,
    /// Neutral working directory the subprocess runs in. Should NOT contain a
    /// `CLAUDE.md` that would leak into the system prompt (and cache). A
    /// dedicated dir like `/tmp/enricher-cwd/` is ideal.
    pub neutral_cwd: PathBuf,
}

impl Default for ClaudeCliConfig {
    fn default() -> Self {
        Self {
            cli_path: PathBuf::from("claude"),
            model: "claude-opus-4-6".to_string(),
            per_call_budget_usd: 0.50,
            timeout: Duration::from_secs(90),
            neutral_cwd: PathBuf::from("/tmp"),
        }
    }
}

pub struct ClaudeCliProvider {
    cfg: ClaudeCliConfig,
}

impl ClaudeCliProvider {
    pub fn new(cfg: ClaudeCliConfig) -> Self {
        Self { cfg }
    }

    /// The JSON schema handed to `--json-schema`. Kept identical across calls
    /// for prompt caching. `discovered_codes` uses object form here (rather
    /// than the Rust tuple) so the LLM sees clearly-named fields.
    fn output_schema() -> &'static str {
        r#"{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "name": { "type": ["string", "null"] },
    "description": { "type": ["string", "null"] },
    "tags": { "type": "array", "items": { "type": "string" } },
    "category": { "type": ["string", "null"] },
    "metadata_additions": { "type": "object" },
    "discovered_codes": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "code_type": { "type": "string" },
          "value": { "type": "string" }
        },
        "required": ["code_type", "value"]
      }
    },
    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
    "reasoning": { "type": ["string", "null"] }
  },
  "required": ["confidence"]
}"#
    }

    /// Stable instructions. Changes here invalidate the whole prompt cache.
    fn prompt_prefix() -> &'static str {
        r#"You are an AI assistant that enriches items in a personal household-inventory system. For each item you are given one or more photos plus optional external codes (ISBN, UPC, EAN, ASIN, etc.), and you must produce structured metadata as JSON.

RULES:
- Output JSON that matches the schema exactly. No commentary before or after.
- Use the Read tool to open and view each image listed in the prompt. Do not use any other tools.
- 'name' should be a concise canonical title (<=80 chars).
- 'description' is 1-3 sentences describing what the item is and its key visible attributes.
- 'tags' must be lowercase. Use only tags from the Allowed Tags list unless explicitly told new tags are permitted, in which case lowercase-hyphenated new tags are acceptable.
- 'category' must be one of the Allowed Categories, or null.
- For books (identified by an ISBN or a clear book-cover appearance), populate metadata_additions.book = { "title", "authors" (array), "publisher", "year", "isbn" }.
- For branded products, populate metadata_additions.product = { "brand", "model", "color" } where applicable.
- Only report 'discovered_codes' for ISBN/UPC/EAN codes you can CLEARLY READ in an image. Never invent or guess them.
- 'confidence' is a scalar in [0, 1] reflecting your overall certainty that the output is correct. Use values below 0.5 when unsure.
- If the item's current fields indicate a user has already edited it, prefer conservative suggestions and lower confidence — your output will be reviewed, not auto-applied.
"#
    }

    fn build_prompt(input: &EnrichmentInput, image_paths: &[PathBuf]) -> String {
        let mut s = String::new();
        s.push_str(Self::prompt_prefix());
        s.push('\n');
        s.push_str("ITEM CONTEXT:\n");
        let existing = |opt: &Option<String>| -> String {
            opt.clone().unwrap_or_else(|| "(empty)".to_string())
        };
        s.push_str(&format!("- Existing name: {}\n", existing(&input.existing_name)));
        s.push_str(&format!(
            "- Existing description: {}\n",
            existing(&input.existing_description)
        ));
        s.push_str(&format!(
            "- Existing category: {}\n",
            existing(&input.existing_category)
        ));
        s.push_str(&format!(
            "- Existing tags: {}\n",
            if input.existing_tags.is_empty() {
                "[]".to_string()
            } else {
                format!("[{}]", input.existing_tags.join(", "))
            }
        ));
        if input.external_codes.is_empty() {
            s.push_str("- External codes: (none)\n");
        } else {
            s.push_str("- External codes:\n");
            for (t, v) in &input.external_codes {
                s.push_str(&format!("  * {t}: {v}\n"));
            }
        }
        if let Some(PresetHint {
            is_container,
            container_type_name,
        }) = &input.preset_hint
        {
            s.push_str(&format!(
                "- Preset hint: kind={:?} is_container={}\n",
                container_type_name.as_deref().unwrap_or("(unknown)"),
                is_container
            ));
        }
        s.push_str(&format!("- User-edited: {}\n", input.user_edited));
        s.push_str(&format!(
            "- Allowed categories: [{}]\n",
            input.available_categories.join(", ")
        ));
        s.push_str(&format!(
            "- Allowed tags: [{}]\n",
            input.available_tags.join(", ")
        ));
        s.push_str(&format!(
            "- New tags allowed: {}\n",
            input.allow_new_tags
        ));
        if image_paths.is_empty() {
            s.push_str("- Images: (none provided — rely on external codes / context)\n");
        } else {
            s.push_str("- Images (read each with the Read tool):\n");
            for p in image_paths {
                s.push_str(&format!("  * {}\n", p.display()));
            }
        }
        s.push_str("\nRespond with the JSON object now.");
        s
    }

    /// Write images from `EnrichmentInput` to the scratch dir and return the
    /// paths to include in the prompt.
    async fn stage_images(
        scratch_dir: &Path,
        input: &EnrichmentInput,
    ) -> Result<Vec<PathBuf>, EnrichmentError> {
        let mut paths = Vec::with_capacity(input.images.len());
        for (i, img) in input.images.iter().enumerate() {
            // Preserve the original extension so claude's Read tool treats it as an image.
            let ext = img
                .item_relative_path
                .rsplit('.')
                .next()
                .filter(|e| !e.is_empty() && e.len() <= 5)
                .unwrap_or("jpg");
            let p = scratch_dir.join(format!("image_{i}.{ext}"));
            let mut f = tokio::fs::File::create(&p).await?;
            f.write_all(&img.bytes).await?;
            f.flush().await?;
            paths.push(p);
        }
        Ok(paths)
    }

    /// Invoke `claude -p ...` and return parsed stdout JSON. Maps known
    /// error messages (auth, spend cap) to typed [`EnrichmentError`]s.
    async fn invoke(
        &self,
        scratch_dir: &Path,
        prompt: &str,
    ) -> Result<ClaudeCliResult, EnrichmentError> {
        let mut cmd = Command::new(&self.cfg.cli_path);
        cmd.arg("-p")
            .arg("--output-format")
            .arg("json")
            .arg("--permission-mode")
            .arg("bypassPermissions")
            .arg("--add-dir")
            .arg(scratch_dir)
            .arg("--allowedTools")
            .arg("Read")
            .arg("--model")
            .arg(&self.cfg.model)
            .arg("--json-schema")
            .arg(Self::output_schema());
        if self.cfg.per_call_budget_usd > 0.0 {
            cmd.arg("--max-budget-usd")
                .arg(format!("{:.4}", self.cfg.per_call_budget_usd));
        }
        cmd.arg(prompt);
        cmd.current_dir(&self.cfg.neutral_cwd);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd
            .spawn()
            .map_err(|e| EnrichmentError::Invocation(format!("spawn claude: {e}")))?;

        let output = match tokio::time::timeout(self.cfg.timeout, child.wait_with_output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(EnrichmentError::Invocation(format!("wait: {e}"))),
            Err(_) => return Err(EnrichmentError::Timeout(self.cfg.timeout.as_secs())),
        };

        // `claude -p --output-format json` ALWAYS prints a JSON object — on
        // success and on most errors. Non-JSON stdout ⇒ subprocess crashed or
        // exited before writing.
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        let parsed: ClaudeCliResult = match serde_json::from_str(stdout.trim()) {
            Ok(p) => p,
            Err(e) => {
                return Err(EnrichmentError::Invocation(format!(
                    "non-JSON stdout from claude (exit={}, stderr_len={}): {e}",
                    output.status,
                    stderr.len()
                )));
            }
        };

        if parsed.is_error {
            let msg = parsed.result.as_deref().unwrap_or("(no result message)");
            // Auth failure is distinguishable only by message text.
            if msg.contains("Not logged in")
                || msg.contains("authentication")
                || msg.contains("ANTHROPIC_API_KEY")
            {
                return Err(EnrichmentError::NotAuthenticated(msg.to_string()));
            }
            if msg.contains("budget") || msg.contains("max-budget") {
                return Err(EnrichmentError::SpendCapped(msg.to_string()));
            }
            return Err(EnrichmentError::Invocation(format!(
                "claude reported error: {msg}"
            )));
        }

        Ok(parsed)
    }
}

#[async_trait::async_trait]
impl EnrichmentProvider for ClaudeCliProvider {
    fn name(&self) -> &'static str {
        "claude_cli"
    }

    fn model_version(&self) -> String {
        format!("claude_cli:{}", self.cfg.model)
    }

    async fn enrich(&self, input: EnrichmentInput) -> Result<EnrichmentOutput, EnrichmentError> {
        // Scratch dir auto-deletes on drop (any return path).
        let tmp = TempDir::new_in("/tmp")
            .map_err(|e| EnrichmentError::Invocation(format!("tempdir: {e}")))?;
        let image_paths = Self::stage_images(tmp.path(), &input).await?;
        let prompt = Self::build_prompt(&input, &image_paths);
        let result = self.invoke(tmp.path(), &prompt).await?;

        let raw = result.structured_output.ok_or_else(|| {
            EnrichmentError::BadOutput(
                "claude returned success but no structured_output field".into(),
            )
        })?;

        let wire: WireOutput = serde_json::from_value(raw)
            .map_err(|e| EnrichmentError::BadOutput(format!("parse structured_output: {e}")))?;

        Ok(EnrichmentOutput {
            name: wire.name.filter(|s| !s.is_empty()),
            description: wire.description.filter(|s| !s.is_empty()),
            tags: wire.tags,
            category: wire.category.filter(|s| !s.is_empty()),
            metadata_additions: wire.metadata_additions,
            discovered_codes: wire
                .discovered_codes
                .into_iter()
                .map(|c| (c.code_type, c.value))
                .collect(),
            confidence: wire.confidence.clamp(0.0, 1.0),
            reasoning: wire.reasoning.filter(|s| !s.is_empty()),
        })
    }
}

// ── Wire types for parsing the CLI's JSON envelope ─────────────────────────

/// Subset of fields emitted by `claude -p --output-format json` that we care
/// about. The CLI emits a rich envelope with usage, cost, and tool calls; we
/// ignore most of it.
#[derive(Debug, Deserialize)]
struct ClaudeCliResult {
    #[serde(default)]
    is_error: bool,
    /// Free-text message: empty on structured-output runs, error description
    /// on failures.
    #[serde(default)]
    result: Option<String>,
    /// The schema-validated JSON object, present when `--json-schema` succeeds.
    #[serde(default)]
    structured_output: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct WireOutput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    metadata_additions: serde_json::Value,
    #[serde(default)]
    discovered_codes: Vec<WireCode>,
    confidence: f32,
    #[serde(default)]
    reasoning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WireCode {
    code_type: String,
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use uuid::Uuid;

    /// Write a bash script to `path` that prints `stdout` verbatim and exits 0.
    /// Used to stub the `claude` binary in tests.
    fn write_fake_claude(path: &Path, stdout_literal: &str) {
        // Dollar-sign literal would need escaping; the JSON we emit doesn't
        // contain `$`. `printf '%s'` preserves formatting exactly.
        let script = format!(
            "#!/usr/bin/env bash\nprintf '%s' {}\n",
            shell_escape(stdout_literal)
        );
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn shell_escape(s: &str) -> String {
        // Wrap in single quotes, escape existing single quotes.
        let mut out = String::with_capacity(s.len() + 2);
        out.push('\'');
        for c in s.chars() {
            if c == '\'' {
                out.push_str("'\\''");
            } else {
                out.push(c);
            }
        }
        out.push('\'');
        out
    }

    fn sample_input() -> EnrichmentInput {
        EnrichmentInput {
            item_id: Uuid::nil(),
            task_id: Uuid::nil(),
            existing_name: None,
            existing_description: None,
            existing_tags: vec![],
            existing_category: None,
            existing_metadata: serde_json::json!({}),
            external_codes: vec![("ISBN".into(), "978-0-201-61622-4".into())],
            preset_hint: Some(PresetHint {
                is_container: false,
                container_type_name: Some("Book".into()),
            }),
            images: vec![],
            available_categories: vec!["Books".into(), "Electronics".into()],
            available_tags: vec!["programming".into(), "fiction".into()],
            allow_new_tags: false,
            user_edited: false,
        }
    }

    #[test]
    fn prompt_contains_schema_and_context() {
        let input = sample_input();
        let prompt = ClaudeCliProvider::build_prompt(&input, &[PathBuf::from("/tmp/img_0.jpg")]);
        assert!(prompt.contains("978-0-201-61622-4"), "ISBN not in prompt");
        assert!(prompt.contains("Books"), "category list missing");
        assert!(prompt.contains("programming"), "tag list missing");
        assert!(prompt.contains("/tmp/img_0.jpg"), "image path missing");
        assert!(prompt.contains("User-edited: false"), "user_edited flag missing");
    }

    #[test]
    fn prefix_is_byte_stable() {
        // Cache-hit guarantee: prefix must be identical across calls.
        let a = ClaudeCliProvider::prompt_prefix();
        let b = ClaudeCliProvider::prompt_prefix();
        assert_eq!(a, b);
        assert!(a.contains("Respond with the JSON object") || a.contains("RULES"));
    }

    #[test]
    fn schema_is_valid_json() {
        let v: serde_json::Value =
            serde_json::from_str(ClaudeCliProvider::output_schema()).expect("valid JSON schema");
        assert_eq!(v["type"], "object");
        assert!(v["properties"]["confidence"].is_object());
    }

    #[tokio::test]
    async fn enrich_parses_structured_output() {
        let tmp = TempDir::new().unwrap();
        let fake = tmp.path().join("fake-claude");
        // Minimal successful response with structured_output populated.
        let canned = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "result": "",
            "structured_output": {
                "name": "The Pragmatic Programmer",
                "description": "A classic book on software craftsmanship.",
                "tags": ["programming", "classic"],
                "category": "Books",
                "metadata_additions": {
                    "book": {
                        "title": "The Pragmatic Programmer",
                        "authors": ["Andrew Hunt", "David Thomas"],
                        "year": 1999
                    }
                },
                "discovered_codes": [
                    {"code_type": "ISBN", "value": "978-0-201-61622-4"}
                ],
                "confidence": 0.92,
                "reasoning": "matched ISBN"
            }
        })
        .to_string();
        write_fake_claude(&fake, &canned);

        let provider = ClaudeCliProvider::new(ClaudeCliConfig {
            cli_path: fake,
            neutral_cwd: tmp.path().to_path_buf(),
            per_call_budget_usd: 0.0,
            timeout: Duration::from_secs(5),
            ..Default::default()
        });

        let out = provider.enrich(sample_input()).await.expect("enrich ok");
        assert_eq!(out.name.as_deref(), Some("The Pragmatic Programmer"));
        assert_eq!(out.tags, vec!["programming", "classic"]);
        assert_eq!(out.category.as_deref(), Some("Books"));
        assert_eq!(out.discovered_codes.len(), 1);
        assert_eq!(out.discovered_codes[0].0, "ISBN");
        assert!((out.confidence - 0.92).abs() < 1e-6);
        assert_eq!(out.metadata_additions["book"]["year"], 1999);
    }

    #[tokio::test]
    async fn enrich_maps_not_logged_in_error() {
        let tmp = TempDir::new().unwrap();
        let fake = tmp.path().join("fake-claude");
        let canned = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login"}"#;
        write_fake_claude(&fake, canned);

        let provider = ClaudeCliProvider::new(ClaudeCliConfig {
            cli_path: fake,
            neutral_cwd: tmp.path().to_path_buf(),
            per_call_budget_usd: 0.0,
            timeout: Duration::from_secs(5),
            ..Default::default()
        });

        let err = provider.enrich(sample_input()).await.unwrap_err();
        assert!(
            matches!(err, EnrichmentError::NotAuthenticated(_)),
            "wrong variant: {err:?}"
        );
        assert!(!err.is_retryable(), "auth errors must not retry");
    }

    #[tokio::test]
    async fn enrich_rejects_non_json_stdout() {
        let tmp = TempDir::new().unwrap();
        let fake = tmp.path().join("fake-claude");
        write_fake_claude(&fake, "garbage not json");

        let provider = ClaudeCliProvider::new(ClaudeCliConfig {
            cli_path: fake,
            neutral_cwd: tmp.path().to_path_buf(),
            per_call_budget_usd: 0.0,
            timeout: Duration::from_secs(5),
            ..Default::default()
        });

        let err = provider.enrich(sample_input()).await.unwrap_err();
        assert!(matches!(err, EnrichmentError::Invocation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn enrich_missing_structured_output_is_bad_output() {
        let tmp = TempDir::new().unwrap();
        let fake = tmp.path().join("fake-claude");
        // Success but no structured_output at all.
        write_fake_claude(
            &fake,
            r#"{"type":"result","subtype":"success","is_error":false,"result":""}"#,
        );

        let provider = ClaudeCliProvider::new(ClaudeCliConfig {
            cli_path: fake,
            neutral_cwd: tmp.path().to_path_buf(),
            per_call_budget_usd: 0.0,
            timeout: Duration::from_secs(5),
            ..Default::default()
        });

        let err = provider.enrich(sample_input()).await.unwrap_err();
        assert!(matches!(err, EnrichmentError::BadOutput(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn enrich_clamps_confidence() {
        let tmp = TempDir::new().unwrap();
        let fake = tmp.path().join("fake-claude");
        let canned = r#"{"type":"result","is_error":false,"result":"","structured_output":{"confidence":1.5}}"#;
        write_fake_claude(&fake, canned);

        let provider = ClaudeCliProvider::new(ClaudeCliConfig {
            cli_path: fake,
            neutral_cwd: tmp.path().to_path_buf(),
            per_call_budget_usd: 0.0,
            timeout: Duration::from_secs(5),
            ..Default::default()
        });

        let out = provider.enrich(sample_input()).await.unwrap();
        assert!((out.confidence - 1.0).abs() < 1e-6, "got {}", out.confidence);
    }
}

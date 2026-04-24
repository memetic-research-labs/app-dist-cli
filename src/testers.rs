use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::Colorize;
use serde::Deserialize;

use crate::config::Config;

#[derive(Subcommand)]
pub enum TesterCommands {
    /// Add one or more testers to an app
    Add(AddArgs),
    /// List testers for an app
    List {
        /// App ID or slug
        #[arg(long)]
        app: String,
    },
    /// Send download links to testers
    Notify(NotifyArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// App ID or slug
    #[arg(long)]
    pub app: String,
    /// Email address(es) to add (may be specified multiple times)
    #[arg(long, num_args = 1..)]
    pub email: Vec<String>,
    /// Path to a file containing email addresses (CSV, JSON, or YAML)
    #[arg(long)]
    pub file: Option<PathBuf>,
}

#[derive(Args)]
pub struct NotifyArgs {
    /// App ID or slug
    #[arg(long)]
    app: String,

    /// Specific release version (defaults to latest)
    #[arg(long)]
    release: Option<String>,

    /// Send to all testers
    #[arg(long)]
    all_testers: bool,

    /// Specific emails (comma-separated)
    #[arg(long)]
    emails: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

/// Collect all email addresses from the `--email` flags and an optional `--file`.
///
/// Supported file formats (detected by extension):
/// * `.json` – JSON array of strings or array of `{"email": "..."}` objects
/// * `.yaml` / `.yml` – YAML sequence of strings or sequence of `{email: ...}` mappings
/// * `.csv`  – one email per row; a header row named "email" is skipped automatically
pub fn collect_emails(emails: &[String], file: Option<&PathBuf>) -> Result<Vec<String>> {
    let mut result: Vec<String> = emails.to_vec();

    if let Some(path) = file {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Could not read file: {}", path.display()))?;

        match ext.as_str() {
            "json" => {
                let val: serde_json::Value =
                    serde_json::from_str(&raw).context("Invalid JSON in tester file")?;
                parse_json_emails(&val, &mut result)?;
            }
            "yaml" | "yml" => {
                let val: serde_yml::Value =
                    serde_yml::from_str(&raw).context("Invalid YAML in tester file")?;
                parse_yaml_emails(&val, &mut result)?;
            }
            "csv" => {
                parse_csv_emails(&raw, &mut result)?;
            }
            other => {
                anyhow::bail!(
                    "Unsupported file extension '{}'. Use .json, .yaml, .yml, or .csv",
                    other
                );
            }
        }
    }

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    result.retain(|e| seen.insert(e.to_lowercase()));

    if result.is_empty() {
        anyhow::bail!("No email addresses provided. Use --email or --file.");
    }

    Ok(result)
}

fn parse_json_emails(val: &serde_json::Value, out: &mut Vec<String>) -> Result<()> {
    let arr = val
        .as_array()
        .context("JSON tester file must be a top-level array")?;
    for item in arr {
        if let Some(s) = item.as_str() {
            out.push(s.to_string());
        } else if let Some(e) = item.get("email").and_then(|v| v.as_str()) {
            out.push(e.to_string());
        } else {
            anyhow::bail!("JSON array items must be strings or objects with an \"email\" key");
        }
    }
    Ok(())
}

fn parse_yaml_emails(val: &serde_yml::Value, out: &mut Vec<String>) -> Result<()> {
    let arr = val
        .as_sequence()
        .context("YAML tester file must be a top-level sequence")?;
    for item in arr {
        if let Some(s) = item.as_str() {
            out.push(s.to_string());
        } else if let Some(e) = item
            .get("email")
            .and_then(|v| v.as_str())
        {
            out.push(e.to_string());
        } else {
            anyhow::bail!("YAML sequence items must be strings or mappings with an \"email\" key");
        }
    }
    Ok(())
}

fn parse_csv_emails(raw: &str, out: &mut Vec<String>) -> Result<()> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(raw.as_bytes());

    // Check whether the first column header is "email" (case-insensitive) or something else.
    // If the header is "email" we read that column; otherwise we assume column 0 is the address.
    let headers = rdr.headers().context("Could not read CSV headers")?.clone();
    let email_col = headers
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case("email"))
        .unwrap_or(0);

    for record in rdr.records() {
        let record = record.context("Invalid CSV record")?;
        if let Some(field) = record.get(email_col) {
            let trimmed = field.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
        }
    }

    // If there were no headers (single-column file with no header), the csv crate
    // still reads the first line as headers. Re-check: if none of the header values
    // look like an email skip re-adding them (they were consumed as headers already).
    // For files that have NO header row at all, re-parse without headers.
    if out.is_empty() {
        let mut rdr2 = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(raw.as_bytes());
        for record in rdr2.records() {
            let record = record.context("Invalid CSV record")?;
            if let Some(field) = record.get(0) {
                let trimmed = field.trim().to_string();
                if !trimmed.is_empty() {
                    out.push(trimmed);
                }
            }
        }
    }

    Ok(())
}

pub async fn run(cfg: &Config, cmd: TesterCommands) -> Result<()> {
    let api_key = cfg.require_api_key()?;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", api_key);

    match cmd {
        TesterCommands::Add(args) => {
            let emails = collect_emails(&args.email, args.file.as_ref())?;

            let mut added = 0usize;
            let mut failed = 0usize;

            for email in &emails {
                let resp = client
                    .post(format!("{}/api/v1/apps/{}/testers", cfg.api_url, args.app))
                    .header("Authorization", &auth)
                    .json(&serde_json::json!({ "email": email }))
                    .send()
                    .await?;

                if resp.status().is_success() {
                    println!(
                        "{} Added tester {} to app {}",
                        "✓".green().bold(),
                        email.blue(),
                        args.app
                    );
                    added += 1;
                } else {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    eprintln!(
                        "{} Could not add {} – {} ({})",
                        "✗".red().bold(),
                        email,
                        body,
                        status
                    );
                    failed += 1;
                }
            }

            if emails.len() > 1 {
                println!(
                    "\n{} {} added, {} failed",
                    "Summary:".bold(),
                    added,
                    failed
                );
            }

            if failed > 0 {
                anyhow::bail!("{} tester(s) could not be added", failed);
            }
        }

        TesterCommands::List { app } => {
            let resp = client
                .get(format!("{}/api/v1/apps/{}/testers", cfg.api_url, app))
                .header("Authorization", &auth)
                .send()
                .await?
                .error_for_status()?;

            let data: serde_json::Value = resp.json().await?;
            let testers = data.get("testers").and_then(|t| t.as_array());

            match testers {
                Some(arr) if !arr.is_empty() => {
                    println!(
                        "{:<40} {:<15} {}",
                        "EMAIL".dimmed(),
                        "GRANTS".dimmed(),
                        "ADDED".dimmed(),
                    );
                    for t in arr {
                        let email = t.get("email").and_then(|e| e.as_str()).unwrap_or("?");
                        let grants = t.get("grant_count").and_then(|g| g.as_u64()).unwrap_or(0);
                        let added = t.get("created_at").and_then(|a| a.as_str()).unwrap_or("?");
                        let added_short = added.get(..10).unwrap_or(added);
                        println!("{:<40} {:<15} {}", email, grants, added_short);
                    }
                }
                _ => {
                    println!("No testers yet. Add one with:");
                    println!("  app-dist testers add --app {} --email alice@example.com", app);
                }
            }
        }

        TesterCommands::Notify(args) => {
            if !args.all_testers && args.emails.is_none() {
                anyhow::bail!("Specify --all-testers or --emails");
            }

            let mut body = serde_json::json!({
                "release": args.release.unwrap_or_else(|| "latest".to_string()),
            });

            if let Some(emails) = &args.emails {
                let list: Vec<&str> = emails.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                body["emails"] = serde_json::json!(list);
            }

            let resp = client
                .post(format!("{}/api/v1/apps/{}/grants/batch", cfg.api_url, args.app))
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("{} {} ({})", "Error:".red().bold(), body, status);
            }

            let data: serde_json::Value = resp.json().await?;
            let sent = data.get("grants_created").and_then(|g| g.as_u64()).unwrap_or(0);
            let emailed = data.get("emails_sent").and_then(|e| e.as_u64()).unwrap_or(0);
            println!("{} {} grant(s) created, {} email(s) sent", "✓".green().bold(), sent, emailed);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── collect_emails: inline --email flags ──────────────────────────────────

    #[test]
    fn test_collect_emails_single() {
        let emails = collect_emails(&["alice@example.com".to_string()], None).unwrap();
        assert_eq!(emails, vec!["alice@example.com"]);
    }

    #[test]
    fn test_collect_emails_multiple() {
        let emails = collect_emails(
            &[
                "alice@example.com".to_string(),
                "bob@example.com".to_string(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(emails, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_collect_emails_dedup() {
        let emails = collect_emails(
            &[
                "alice@example.com".to_string(),
                "Alice@Example.COM".to_string(),
                "bob@example.com".to_string(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(emails, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_collect_emails_empty_returns_error() {
        let err = collect_emails(&[], None).unwrap_err();
        assert!(err.to_string().contains("No email addresses provided"));
    }

    // ── JSON parsing ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_json_string_array() {
        let mut out = Vec::new();
        let val: serde_json::Value =
            serde_json::from_str(r#"["alice@example.com","bob@example.com"]"#).unwrap();
        parse_json_emails(&val, &mut out).unwrap();
        assert_eq!(out, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_parse_json_object_array() {
        let mut out = Vec::new();
        let val: serde_json::Value = serde_json::from_str(
            r#"[{"email":"alice@example.com"},{"email":"bob@example.com"}]"#,
        )
        .unwrap();
        parse_json_emails(&val, &mut out).unwrap();
        assert_eq!(out, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_parse_json_not_array_returns_error() {
        let mut out = Vec::new();
        let val: serde_json::Value = serde_json::from_str(r#"{"email":"alice@example.com"}"#).unwrap();
        assert!(parse_json_emails(&val, &mut out).is_err());
    }

    // ── YAML parsing ──────────────────────────────────────────────────────────

    #[test]
    fn test_parse_yaml_string_sequence() {
        let mut out = Vec::new();
        let val: serde_yml::Value =
            serde_yml::from_str("- alice@example.com\n- bob@example.com\n").unwrap();
        parse_yaml_emails(&val, &mut out).unwrap();
        assert_eq!(out, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_parse_yaml_object_sequence() {
        let mut out = Vec::new();
        let val: serde_yml::Value =
            serde_yml::from_str("- email: alice@example.com\n- email: bob@example.com\n").unwrap();
        parse_yaml_emails(&val, &mut out).unwrap();
        assert_eq!(out, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_parse_yaml_not_sequence_returns_error() {
        let mut out = Vec::new();
        let val: serde_yml::Value = serde_yml::from_str("email: alice@example.com\n").unwrap();
        assert!(parse_yaml_emails(&val, &mut out).is_err());
    }

    // ── CSV parsing ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_csv_with_header() {
        let mut out = Vec::new();
        parse_csv_emails("email\nalice@example.com\nbob@example.com\n", &mut out).unwrap();
        assert_eq!(out, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_parse_csv_with_name_and_email_columns() {
        let mut out = Vec::new();
        parse_csv_emails(
            "name,email\nAlice,alice@example.com\nBob,bob@example.com\n",
            &mut out,
        )
        .unwrap();
        assert_eq!(out, vec!["alice@example.com", "bob@example.com"]);
    }

    #[test]
    fn test_collect_emails_from_json_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("testers_test.json");
        std::fs::write(&path, r#"["alice@example.com","bob@example.com"]"#).unwrap();
        let emails = collect_emails(&[], Some(&path)).unwrap();
        assert_eq!(emails, vec!["alice@example.com", "bob@example.com"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_collect_emails_from_yaml_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("testers_test.yaml");
        std::fs::write(&path, "- alice@example.com\n- bob@example.com\n").unwrap();
        let emails = collect_emails(&[], Some(&path)).unwrap();
        assert_eq!(emails, vec!["alice@example.com", "bob@example.com"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_collect_emails_from_csv_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("testers_test.csv");
        std::fs::write(&path, "email\nalice@example.com\nbob@example.com\n").unwrap();
        let emails = collect_emails(&[], Some(&path)).unwrap();
        assert_eq!(emails, vec!["alice@example.com", "bob@example.com"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_collect_emails_merge_inline_and_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("testers_merge_test.json");
        std::fs::write(&path, r#"["bob@example.com","carol@example.com"]"#).unwrap();
        let emails =
            collect_emails(&["alice@example.com".to_string()], Some(&path)).unwrap();
        assert_eq!(
            emails,
            vec!["alice@example.com", "bob@example.com", "carol@example.com"]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_collect_emails_unsupported_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("testers.txt");
        std::fs::write(&path, "alice@example.com\n").unwrap();
        let err = collect_emails(&[], Some(&path)).unwrap_err();
        assert!(err.to_string().contains("Unsupported file extension"));
        let _ = std::fs::remove_file(&path);
    }
}

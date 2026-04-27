use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use serde::Deserialize;

use crate::config::Config;

#[derive(Subcommand)]
pub enum TesterCommands {
    /// Add a tester to an app
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
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub email: String,
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

pub async fn run(cfg: &Config, cmd: TesterCommands) -> Result<()> {
    let api_key = cfg.require_api_key()?;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", api_key);

    match cmd {
        TesterCommands::Add(args) => {
            let resp = client
                .post(format!("{}/api/v1/apps/{}/testers", cfg.api_url, args.app))
                .header("Authorization", &auth)
                .json(&serde_json::json!({
                    "email": args.email,
                }))
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("{} {} ({})", "Error:".red().bold(), body, status);
            }

            println!(
                "{} Added tester {} to app {}",
                "✓".green().bold(),
                args.email.blue(),
                args.app
            );
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
                    println!(
                        "  app-dist testers add --app {} --email alice@example.com",
                        app
                    );
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
                let list: Vec<&str> = emails
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                body["emails"] = serde_json::json!(list);
            }

            let resp = client
                .post(format!(
                    "{}/api/v1/apps/{}/grants/batch",
                    cfg.api_url, args.app
                ))
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
            let sent = data
                .get("grants_created")
                .and_then(|g| g.as_u64())
                .unwrap_or(0);
            let emailed = data
                .get("emails_sent")
                .and_then(|e| e.as_u64())
                .unwrap_or(0);
            println!(
                "{} {} grant(s) created, {} email(s) sent",
                "✓".green().bold(),
                sent,
                emailed
            );
        }
    }

    Ok(())
}

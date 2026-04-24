use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use serde::Deserialize;

use crate::config::Config;

#[derive(Subcommand)]
pub enum AppCommands {
    /// Create a new app
    Create(CreateArgs),
    /// List all your apps
    List,
    /// Show app details
    Info {
        /// App ID or slug
        app: String,
    },
}

#[derive(Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub bundle_id: Option<String>,
    #[arg(long)]
    pub homepage_url: Option<String>,
    #[arg(long)]
    pub support_email: Option<String>,
}

#[derive(Deserialize)]
struct AppResponse {
    id: String,
    slug: String,
    display_name: String,
    bundle_id: Option<String>,
    pricing_type: String,
    sparkle_enabled: u8,
    created_at: String,
}

#[derive(Deserialize)]
struct CreateResponse {
    app: AppResponse,
}

#[derive(Deserialize)]
struct ListResponse {
    apps: Vec<AppResponse>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

pub async fn run(cfg: &Config, cmd: AppCommands) -> Result<()> {
    let api_key = cfg.require_api_key()?;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", api_key);

    match cmd {
        AppCommands::Create(args) => {
            let mut body = serde_json::json!({
                "display_name": args.name,
            });
            if let Some(bid) = &args.bundle_id {
                body["bundle_id"] = serde_json::json!(bid);
            }
            if let Some(url) = &args.homepage_url {
                body["homepage_url"] = serde_json::json!(url);
            }
            if let Some(email) = &args.support_email {
                body["support_email"] = serde_json::json!(email);
            }

            let resp = client
                .post(format!("{}/api/v1/apps", cfg.api_url))
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("{} {} ({})", "Error:".red().bold(), body, status);
            }

            let data: CreateResponse = resp.json().await?;
            println!("{} Created app: {} (slug: {})", "✓".green().bold(), data.app.display_name, data.app.slug);
            println!("  {} {}", "ID:".dimmed(), data.app.id);
            if let Some(bid) = &data.app.bundle_id {
                println!("  {} {}", "Bundle ID:".dimmed(), bid);
            }
        }

        AppCommands::List => {
            let resp = client
                .get(format!("{}/api/v1/apps", cfg.api_url))
                .header("Authorization", &auth)
                .send()
                .await?
                .error_for_status()?;

            let data: ListResponse = resp.json().await?;

            if data.apps.is_empty() {
                println!("No apps yet. Create one with:");
                println!("  app-dist app create --name \"My App\"");
                return Ok(());
            }

            println!(
                "{:<36} {:<20} {:<10} {}",
                "ID".dimmed(),
                "SLUG".dimmed(),
                "PLAN".dimmed(),
                "CREATED".dimmed(),
            );
            for app in &data.apps {
                println!(
                    "{:<36} {:<20} {:<10} {}",
                    truncate_id(&app.id),
                    app.slug,
                    app.pricing_type,
                    truncate_date(&app.created_at),
                );
            }
        }

        AppCommands::Info { app } => {
            let resp = client
                .get(format!("{}/api/v1/apps/{}", cfg.api_url, app))
                .header("Authorization", &auth)
                .send()
                .await?
                .error_for_status()?;

            let data: AppResponse = resp.json().await?;
            println!("{} {} ({})", "App:".green().bold(), data.display_name, data.slug);
            println!("  {} {}", "ID:".dimmed(), data.id);
            if let Some(bid) = &data.bundle_id {
                println!("  {} {}", "Bundle ID:".dimmed(), bid);
            }
            println!("  {} {}", "Pricing:".dimmed(), data.pricing_type);
            println!("  {} {}", "Sparkle:".dimmed(), if data.sparkle_enabled == 1 { "enabled" } else { "disabled" });
            println!("  {} {}", "Created:".dimmed(), data.created_at);
        }
    }

    Ok(())
}

fn truncate_id(id: &str) -> String {
    if id.len() > 36 {
        format!("{}…", &id[..35])
    } else {
        id.to_string()
    }
}

fn truncate_date(date: &str) -> String {
    date.get(..10).unwrap_or(date).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_id_short() {
        assert_eq!(truncate_id("abc"), "abc");
    }

    #[test]
    fn test_truncate_id_exact() {
        let id = "a".repeat(36);
        assert_eq!(truncate_id(&id), id);
    }

    #[test]
    fn test_truncate_id_long() {
        let id = "a".repeat(50);
        assert_eq!(truncate_id(&id), format!("{}…", "a".repeat(35)));
    }

    #[test]
    fn test_truncate_date_full() {
        assert_eq!(truncate_date("2026-04-23T12:00:00"), "2026-04-23");
    }

    #[test]
    fn test_truncate_date_short() {
        assert_eq!(truncate_date("short"), "short");
    }

    #[test]
    fn test_truncate_date_empty() {
        assert_eq!(truncate_date(""), "");
    }
}

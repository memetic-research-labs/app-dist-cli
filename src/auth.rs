use anyhow::Result;
use clap::Args;
use colored::Colorize;
use serde::Deserialize;

use crate::config::{set_keychain_key, Config};

#[derive(Args)]
pub struct LoginArgs {
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
struct WhoamiResponse {
    id: String,
    email: String,
    plan: String,
    app_count: u32,
}

#[derive(Deserialize)]
struct RotateKeyResponse {
    api_key: String,
    api_key_prefix: String,
}

pub async fn login(cfg: &Config, args: LoginArgs) -> Result<()> {
    let api_key = match args.api_key {
        Some(key) => key,
        None => dialoguer::Password::new()
            .with_prompt("API key (paste from app-dist.com/dashboard)")
            .interact()?,
    };

    if !api_key.starts_with("apd_") {
        anyhow::bail!("API key should start with apd_");
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/v1/auth/whoami", cfg.api_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Invalid API key ({}): {}", status, body);
    }

    let whoami: WhoamiResponse = resp.json().await?;

    set_keychain_key(&api_key)?;
    println!(
        "{} Authenticated as {} (plan: {})",
        "✓".green().bold(),
        whoami.email.blue(),
        whoami.plan
    );
    println!("  {} app(s) on this account", whoami.app_count);

    Ok(())
}

pub async fn whoami(cfg: &Config) -> Result<()> {
    let api_key = cfg.require_api_key()?;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/v1/auth/whoami", cfg.api_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?
        .error_for_status()?;

    let data: WhoamiResponse = resp.json().await?;
    println!("{} {}", "ID:".dimmed(), data.id);
    println!("{} {}", "Email:".dimmed(), data.email);
    println!("{} {}", "Plan:".dimmed(), data.plan);
    println!("{} {}", "Apps:".dimmed(), data.app_count);

    Ok(())
}

pub async fn rotate_key(cfg: &Config) -> Result<()> {
    let api_key = cfg.require_api_key()?;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/auth/rotate-key", cfg.api_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?
        .error_for_status()?;

    let data: RotateKeyResponse = resp.json().await?;

    set_keychain_key(&data.api_key)?;
    println!("{} API key rotated", "✓".green().bold());
    println!("  New prefix: {}", data.api_key_prefix.yellow());
    println!(
        "  {} This is the only time you'll see the full key.",
        "Warning:".yellow().bold()
    );
    println!("  Save it somewhere safe if needed, but it's stored in your Keychain.");

    Ok(())
}

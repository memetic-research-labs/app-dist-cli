use anyhow::Result;
use colored::Colorize;
use serde::Deserialize;

use crate::config::Config;

#[derive(Deserialize)]
struct WhoamiResponse {
    #[allow(dead_code)]
    id: String,
    email: String,
    plan: String,
    app_count: u32,
}

#[derive(Deserialize)]
struct AppResponse {
    id: String,
    slug: String,
    display_name: String,
    pricing_type: String,
    created_at: String,
}

#[derive(Deserialize)]
struct ReleaseResponse {
    version: String,
    build_number: u32,
    channel: String,
    #[allow(dead_code)]
    created_at: String,
}

pub async fn show(cfg: &Config) -> Result<()> {
    let api_key = cfg.require_api_key()?;
    let client = reqwest::Client::new();
    let auth = format!("Bearer {}", api_key);

    let resp = client
        .get(format!("{}/api/v1/auth/whoami", cfg.api_url))
        .header("Authorization", &auth)
        .send()
        .await?
        .error_for_status()?;
    let whoami: WhoamiResponse = resp.json().await?;

    let resp = client
        .get(format!("{}/api/v1/apps", cfg.api_url))
        .header("Authorization", &auth)
        .send()
        .await?
        .error_for_status()?;
    let apps_data: serde_json::Value = resp.json().await?;
    let apps: Vec<AppResponse> = apps_data
        .get("apps")
        .and_then(|a| serde_json::from_value(a.clone()).ok())
        .unwrap_or_default();

    println!();
    println!(
        "{}",
        "╔══════════════════════════════════════════╗".dimmed()
    );
    println!("{}", "║          app-dist Dashboard             ║".dimmed());
    println!(
        "{}",
        "╚══════════════════════════════════════════╝".dimmed()
    );
    println!();
    println!("  {} {}", "Account:".dimmed(), whoami.email);
    println!("  {} {}", "Plan:".dimmed(), whoami.plan);
    println!("  {} {}", "Apps:".dimmed(), whoami.app_count);
    println!();

    if apps.is_empty() {
        println!("  {}", "No apps yet.".dimmed());
        println!("  Create one: app-dist app create --name \"My App\"");
    } else {
        println!(
            "  {:<20} {:<12} {:<8} {}",
            "APP".dimmed(),
            "SLUG".dimmed(),
            "TYPE".dimmed(),
            "CREATED".dimmed(),
        );
        for app in &apps {
            println!(
                "  {:<20} {:<12} {:<8} {}",
                truncate(app.display_name.clone(), 20),
                truncate(app.slug.clone(), 12),
                app.pricing_type,
                &app.created_at[..10],
            );
        }

        println!();
        println!("  {}", "Recent Releases:".dimmed());
        let mut found_any = false;
        for app in &apps {
            let resp = client
                .get(format!(
                    "{}/api/v1/apps/{}/releases?limit=1",
                    cfg.api_url, app.id
                ))
                .header("Authorization", &auth)
                .send()
                .await;

            if let Ok(resp) = resp {
                if let Ok(releases_data) = resp.json::<serde_json::Value>().await {
                    let releases: Vec<ReleaseResponse> = releases_data
                        .get("releases")
                        .and_then(|r| serde_json::from_value(r.clone()).ok())
                        .unwrap_or_default();

                    for r in &releases {
                        found_any = true;
                        println!(
                            "  {} {} {} ({}.{})",
                            "•".dimmed(),
                            app.display_name,
                            format!("v{}", r.version).cyan(),
                            r.channel,
                            r.build_number,
                        );
                    }
                }
            }
        }
        if !found_any {
            println!("  {}", "No releases yet.".dimmed());
        }
    }

    println!();

    match whoami.plan.as_str() {
        "free" => {
            println!(
                "  {} You're on the Free plan (1 app, 10 testers, 5 releases/month)",
                "Tip:".yellow()
            );
        }
        _ => {}
    }

    Ok(())
}

fn truncate(s: String, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max - 1])
    } else {
        s
    }
}

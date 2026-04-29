use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::config::Config;

#[derive(Args)]
pub struct ReleaseArgs {
    #[arg(long)]
    pub app: Option<String>,

    #[arg(long)]
    pub version: Option<String>,

    #[arg(long)]
    pub build: Option<u32>,

    #[arg(long)]
    pub archive_path: Option<PathBuf>,

    #[arg(long)]
    pub project: Option<String>,

    #[arg(long)]
    pub scheme: Option<String>,

    #[arg(long, default_value = "dist")]
    pub out_dir: PathBuf,

    #[arg(long)]
    pub skip_notarize: bool,

    #[arg(long)]
    pub skip_sign: bool,

    #[arg(long)]
    pub github_release: bool,

    #[arg(long)]
    pub github_repo: Option<String>,

    #[arg(long)]
    pub github_tag: Option<String>,

    #[arg(long)]
    pub github_title: Option<String>,

    #[arg(long)]
    pub github_notes: Option<PathBuf>,

    #[arg(long)]
    pub github_draft: bool,

    #[arg(long)]
    pub github_prerelease: bool,
}

#[derive(Deserialize)]
struct UploadUrlResponse {
    release_id: String,
    dmg_key: String,
    zip_key: String,
    upload_base_url: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

pub async fn run(cfg: &Config, args: ReleaseArgs) -> Result<()> {
    let api_key = cfg.require_api_key()?;

    let app_id = match &args.app {
        Some(id) => id.clone(),
        None => {
            let yaml = cfg.read_project_config()?;
            parse_yaml_field(&yaml, "app")
                .context("No --app specified and no app-dist.yml found")?
        }
    };

    let scheme = args.scheme.clone().unwrap_or_else(|| {
        parse_yaml_field(&cfg.read_project_config().unwrap_or_default(), "scheme")
            .unwrap_or_default()
    });

    let project = args.project.clone().unwrap_or_else(|| {
        parse_yaml_field(
            &cfg.read_project_config().unwrap_or_default(),
            "xcode_project",
        )
        .unwrap_or_default()
    });

    if scheme.is_empty() || project.is_empty() {
        anyhow::bail!(
            "Scheme and project are required. Set them in app-dist.yml or use --scheme/--project."
        );
    }

    let archive_path = args
        .archive_path
        .unwrap_or_else(|| PathBuf::from("build").join(format!("{}.xcarchive", scheme)));

    let version = args.version.clone().unwrap_or_else(|| {
        parse_plist_value(&archive_path, "CFBundleShortVersionString")
            .unwrap_or_else(|_| "0.0.0".to_string())
    });

    let build_number = args.build.unwrap_or_else(|| {
        parse_plist_value(&archive_path, "CFBundleVersion")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    });

    if build_number == 0 {
        anyhow::bail!(
            "Build number is required. Set --build or ensure the archive has CFBundleVersion."
        );
    }

    let out_dir = &args.out_dir;
    std::fs::create_dir_all(out_dir)?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    if !archive_path.exists() {
        spinner.set_message(format!("Archiving {} ({})...", scheme, "Release"));
        let status = tokio::process::Command::new("xcodebuild")
            .arg("archive")
            .arg("-project")
            .arg(&project)
            .arg("-scheme")
            .arg(&scheme)
            .arg("-configuration")
            .arg("Release")
            .arg("-archivePath")
            .arg(&archive_path)
            .arg("-destination")
            .arg("generic/platform=macOS")
            .arg("-quiet")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("xcodebuild archive failed");
        }
    }

    spinner.set_message("Exporting for Developer ID...");
    let export_dir = PathBuf::from("build").join("export");
    let _ = std::fs::remove_dir_all(&export_dir);
    std::fs::create_dir_all(&export_dir)?;

    let export_plist = export_dir.join("ExportOptions.plist");
    tokio::fs::write(&export_plist,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>developer-id</string>
</dict>
</plist>"#
    ).await?;

    if !args.skip_sign {
        let status = tokio::process::Command::new("xcodebuild")
            .arg("-exportArchive")
            .arg("-archivePath")
            .arg(&archive_path)
            .arg("-exportPath")
            .arg(&export_dir)
            .arg("-exportOptionsPlist")
            .arg(&export_plist)
            .arg("-quiet")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("xcodebuild export failed");
        }
    } else {
        let app_in_archive = find_app_in_archive(&archive_path)?;
        let dest = export_dir.join(app_in_archive.file_name().unwrap());
        tokio::fs::copy(&app_in_archive, &dest).await?;
    }

    let app_path = find_app_in_dir(&export_dir)?;
    let app_name = app_path.file_stem().unwrap().to_string_lossy().to_string();

    let dmg_name = format!("{}-{}-{}.dmg", app_name, version, build_number);
    let dmg_path = out_dir.join(&dmg_name);
    let zip_name = format!("{}-{}-{}.zip", app_name, version, build_number);
    let zip_path = out_dir.join(&zip_name);

    if !args.skip_sign {
        spinner.set_message("Verifying code signature...");
        let status = tokio::process::Command::new("codesign")
            .arg("--verify")
            .arg("--deep")
            .arg("--strict")
            .arg("--verbose=2")
            .arg(&app_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("codesign verification failed");
        }
    }

    spinner.set_message("Creating DMG...");
    let dmg_staging = std::env::temp_dir().join(format!("app-dist-dmg-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dmg_staging)?;
    let _ = copy_dir_recursive(&app_path, &dmg_staging.join(&app_name));
    let status = tokio::process::Command::new("hdiutil")
        .arg("create")
        .arg("-volname")
        .arg(&app_name)
        .arg("-srcfolder")
        .arg(&dmg_staging)
        .arg("-ov")
        .arg("-format")
        .arg("UDZO")
        .arg(&dmg_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("hdiutil create failed");
    }
    let _ = std::fs::remove_dir_all(&dmg_staging);

    if !args.skip_sign && !args.skip_notarize {
        spinner.set_message("Notarizing (this may take a few minutes)...");
        let status = tokio::process::Command::new("xcrun")
            .arg("notarytool")
            .arg("submit")
            .arg(&dmg_path)
            .arg("--wait")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("Notarization failed");
        }

        spinner.set_message("Stapling notarization ticket...");
        let _ = tokio::process::Command::new("xcrun")
            .arg("stapler")
            .arg("staple")
            .arg(&dmg_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
    }

    spinner.set_message("Creating ZIP...");
    let status = tokio::process::Command::new("ditto")
        .arg("-c")
        .arg("-k")
        .arg("--sequesterRsrc")
        .arg("--keepParent")
        .arg(&app_path)
        .arg(&zip_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("ditto (ZIP) failed");
    }

    spinner.set_message("Requesting upload URL...");
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "{}/api/v1/apps/{}/releases/upload-url",
            cfg.api_url, app_id
        ))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "version": version,
            "build_number": build_number,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        spinner.finish_and_clear();
        anyhow::bail!("{} {} ({})", "Error:".red().bold(), body, status);
    }

    let upload_data: UploadUrlResponse = resp.json().await?;

    spinner.set_message("Uploading DMG...");
    let dmg_bytes = tokio::fs::read(&dmg_path).await?;
    let _ = client
        .put(format!(
            "{}/{}",
            upload_data.upload_base_url, upload_data.dmg_key
        ))
        .body(dmg_bytes)
        .send()
        .await?;

    spinner.set_message("Uploading ZIP...");
    let zip_bytes = tokio::fs::read(&zip_path).await?;
    let _ = client
        .put(format!(
            "{}/{}",
            upload_data.upload_base_url, upload_data.zip_key
        ))
        .body(zip_bytes)
        .send()
        .await?;

    spinner.finish_and_clear();

    let dmg_size = std::fs::metadata(&dmg_path)
        .map(|m| format_size(m.len()))
        .unwrap_or_default();
    let zip_size = std::fs::metadata(&zip_path)
        .map(|m| format_size(m.len()))
        .unwrap_or_default();

    let sha_name = format!("{}-{}-{}.sha256", app_name, version, build_number);
    let sha_path = out_dir.join(&sha_name);
    let shasum_status = tokio::process::Command::new("shasum")
        .arg("-a")
        .arg("256")
        .arg(&dmg_name)
        .arg(&zip_name)
        .current_dir(out_dir)
        .stdout(std::process::Stdio::piped())
        .output()
        .await?;
    if shasum_status.status.success() {
        tokio::fs::write(&sha_path, &shasum_status.stdout).await?;
    }

    println!(
        "{} Release v{} (build {}) published!",
        "✓".green().bold(),
        version,
        build_number
    );
    println!("  DMG: {} ({})", dmg_name.cyan(), dmg_size);
    println!("  ZIP: {} ({})", zip_name.cyan(), zip_size);
    println!("  Release ID: {}", upload_data.release_id.dimmed());

    if args.github_release {
        let gh_repo = args.github_repo.clone().unwrap_or_else(|| {
            detect_git_remote_origin().unwrap_or_else(|| "OWNER/REPO".to_string())
        });
        let tag = args
            .github_tag
            .clone()
            .unwrap_or_else(|| format!("v{}", version));
        let title = args
            .github_title
            .clone()
            .unwrap_or_else(|| format!("v{}", version));

        let release_spinner = ProgressBar::new_spinner();
        release_spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        release_spinner.enable_steady_tick(std::time::Duration::from_millis(80));

        release_spinner.set_message(format!("Creating GitHub Release {}...", tag));

        let mut cmd = tokio::process::Command::new("gh");
        cmd.arg("release")
            .arg("create")
            .arg(&tag)
            .arg("--repo")
            .arg(&gh_repo)
            .arg("--title")
            .arg(&title);

        if args.github_draft {
            cmd.arg("--draft");
        }
        if args.github_prerelease {
            cmd.arg("--prerelease");
        }
        if let Some(ref notes_path) = args.github_notes {
            cmd.arg("--notes-file").arg(notes_path);
        } else {
            cmd.arg("--generate-notes");
        }

        cmd.arg(&dmg_path);
        cmd.arg(&zip_path);
        if sha_path.exists() {
            cmd.arg(&sha_path);
        }

        let output = cmd.output().await?;
        release_spinner.finish_and_clear();

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let release_url = stdout
                .lines()
                .find(|l| l.contains("https://github.com"))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| format!("https://github.com/{}/releases/tag/{}", gh_repo, tag));
            println!("  GitHub Release: {}", release_url.cyan());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("{} GitHub release creation failed: {}", "Warning:".yellow().bold(), stderr);
            eprintln!("  Run manually: gh release create {} --repo {} --title {}", tag, gh_repo, title);
        }
    }

    println!();
    println!("{}", "Next: invite testers".dimmed());
    println!(
        "  app-dist testers add --app {} --email alice@example.com",
        app_id
    );

    Ok(())
}

fn detect_git_remote_origin() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        return None;
    }

    let cleaned = url.trim_end_matches(".git");

    let repo = if cleaned.contains(':') && cleaned.starts_with("git@") {
        cleaned.split(':').nth(1)?.to_string()
    } else {
        cleaned.rsplit('/').next()?.to_string()
    };

    if repo.contains('/') {
        Some(repo)
    } else {
        None
    }
}

fn find_app_in_archive(archive: &Path) -> Result<PathBuf> {
    let apps_dir = archive.join("Products").join("Applications");
    for entry in std::fs::read_dir(&apps_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "app") {
            return Ok(path);
        }
    }
    anyhow::bail!("No .app found in archive")
}

fn find_app_in_dir(dir: &PathBuf) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "app") && path.metadata()?.is_dir() {
            return Ok(path);
        }
    }
    anyhow::bail!("No .app found after export")
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn parse_plist_value(archive: &Path, key: &str) -> Result<String> {
    let info_plist = archive.join("Info.plist");
    if !info_plist.exists() {
        let apps_dir = archive.join("Products").join("Applications");
        for entry in std::fs::read_dir(&apps_dir)? {
            let entry = entry?;
            let candidate = entry.path().join("Contents").join("Info.plist");
            if candidate.exists() {
                return parse_plist_value_from(&candidate, key);
            }
        }
        anyhow::bail!("Info.plist not found in archive")
    }
    parse_plist_value_from(&info_plist, key)
}

fn parse_plist_value_from(plist: &PathBuf, key: &str) -> Result<String> {
    let output = std::process::Command::new("/usr/libexec/PlistBuddy")
        .arg("-c")
        .arg(format!("Print :{}", key))
        .arg(plist)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_yaml_field(yaml: &str, field: &str) -> Option<String> {
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{}:", field)) {
            let val = trimmed.trim_start_matches(&format!("{}:", field)).trim();
            let val = val.trim_start_matches('"').trim_end_matches('"');
            return Some(val.to_string());
        }
    }
    None
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(2048), "2 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(52_428_800), "50.0 MB");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "0 B");
    }

    #[test]
    fn test_parse_yaml_field_found() {
        let yaml = "app: my-app\nscheme: MyApp";
        assert_eq!(parse_yaml_field(yaml, "app"), Some("my-app".to_string()));
    }

    #[test]
    fn test_parse_yaml_field_quoted() {
        let yaml = "app: \"my-app\"";
        assert_eq!(parse_yaml_field(yaml, "app"), Some("my-app".to_string()));
    }

    #[test]
    fn test_parse_yaml_field_not_found() {
        let yaml = "app: my-app\nscheme: MyApp";
        assert_eq!(parse_yaml_field(yaml, "channel"), None);
    }

    #[test]
    fn test_parse_yaml_field_empty() {
        assert_eq!(parse_yaml_field("", "app"), None);
    }

    #[test]
    fn test_parse_yaml_field_partial_match() {
        let yaml = "app_name: something";
        assert_eq!(parse_yaml_field(yaml, "app"), None);
    }

    #[test]
    fn test_detect_git_remote_origin_https() {
        let output = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .ok();
        if let Some(output) = output {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !url.is_empty() {
                let result = detect_git_remote_origin();
                assert!(result.is_some(), "Should detect git remote origin");
                let repo = result.unwrap();
                assert!(repo.contains('/'), "Should be in owner/repo format: {}", repo);
                assert!(!repo.contains(':'), "Should not contain colon: {}", repo);
            }
        }
    }

    #[test]
    fn test_release_args_github_flags() {
        use clap::Parser;
        use crate::Cli;
        let cli = Cli::try_parse_from([
            "app-dist", "release",
            "--app", "my-app",
            "--github-release",
            "--github-repo", "owner/repo",
            "--github-tag", "v1.0.0",
            "--github-title", "Release v1.0.0",
            "--github-draft",
            "--github-prerelease",
        ]).unwrap();
        match cli.command {
            crate::Commands::Release(args) => {
                assert!(args.github_release);
                assert_eq!(args.github_repo, Some("owner/repo".to_string()));
                assert_eq!(args.github_tag, Some("v1.0.0".to_string()));
                assert_eq!(args.github_title, Some("Release v1.0.0".to_string()));
                assert!(args.github_draft);
                assert!(args.github_prerelease);
            }
            _ => panic!("expected Release command"),
        }
    }

    #[test]
    fn test_release_args_github_release_false_by_default() {
        use clap::Parser;
        use crate::Cli;
        let cli = Cli::try_parse_from(["app-dist", "release", "--app", "my-app"]).unwrap();
        match cli.command {
            crate::Commands::Release(args) => {
                assert!(!args.github_release);
                assert_eq!(args.github_repo, None);
                assert_eq!(args.github_tag, None);
                assert!(!args.github_draft);
                assert!(!args.github_prerelease);
            }
            _ => panic!("expected Release command"),
        }
    }
}

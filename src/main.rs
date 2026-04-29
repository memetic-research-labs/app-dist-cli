mod app;
mod auth;
mod config;
mod release;
mod status;
mod testers;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "app-dist",
    version,
    about = "Ship macOS betas with one CLI command"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// API server base URL
    #[arg(long, env = "APP_DIST_API_URL", global = true)]
    api_url: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Authenticate with app-dist (stores API key in Keychain)
    Login(auth::LoginArgs),
    /// Show current authenticated developer info
    Whoami,
    /// Rotate your API key (invalidates the old one)
    RotateKey,
    /// Configure Apple signing and notarization credentials
    #[command(name = "setup")]
    SetupSigning,

    /// Manage apps
    #[command(subcommand)]
    App(app::AppCommands),

    /// Build, sign, notarize, and publish a release
    Release(release::ReleaseArgs),

    /// Manage testers and send download links
    #[command(subcommand)]
    Testers(testers::TesterCommands),

    /// Show dashboard overview (apps, recent releases, grant stats)
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let cfg = config::Config::load(cli.api_url.as_deref())?;

    match cli.command {
        Commands::Login(args) => auth::login(&cfg, args).await,
        Commands::Whoami => auth::whoami(&cfg).await,
        Commands::RotateKey => auth::rotate_key(&cfg).await,
        Commands::SetupSigning => config::setup_signing().await,
        Commands::App(cmd) => app::run(&cfg, cmd).await,
        Commands::Release(args) => release::run(&cfg, args).await,
        Commands::Testers(cmd) => testers::run(&cfg, cmd).await,
        Commands::Status => status::show(&cfg).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_login() {
        let cli = Cli::try_parse_from(["app-dist", "login", "apd_test123456789012"]).unwrap();
        match cli.command {
            Commands::Login(args) => {
                assert_eq!(args.api_key, Some("apd_test123456789012".to_string()));
            }
            _ => panic!("expected Login command"),
        }
    }

    #[test]
    fn test_cli_parse_whoami() {
        let cli = Cli::try_parse_from(["app-dist", "whoami"]).unwrap();
        matches!(cli.command, Commands::Whoami);
    }

    #[test]
    fn test_cli_parse_rotate_key() {
        let cli = Cli::try_parse_from(["app-dist", "rotate-key"]).unwrap();
        matches!(cli.command, Commands::RotateKey);
    }

    #[test]
    fn test_cli_parse_setup() {
        let cli = Cli::try_parse_from(["app-dist", "setup"]).unwrap();
        matches!(cli.command, Commands::SetupSigning);
    }

    #[test]
    fn test_cli_parse_app_create() {
        let cli = Cli::try_parse_from([
            "app-dist",
            "app",
            "create",
            "--name",
            "My App",
            "--bundle-id",
            "com.example.myapp",
            "--support-email",
            "support@example.com",
        ])
        .unwrap();
        match cli.command {
            Commands::App(app::AppCommands::Create(args)) => {
                assert_eq!(args.name, "My App");
                assert_eq!(args.bundle_id, Some("com.example.myapp".to_string()));
                assert_eq!(args.support_email, Some("support@example.com".to_string()));
            }
            _ => panic!("expected App Create command"),
        }
    }

    #[test]
    fn test_cli_parse_app_list() {
        let cli = Cli::try_parse_from(["app-dist", "app", "list"]).unwrap();
        matches!(cli.command, Commands::App(app::AppCommands::List));
    }

    #[test]
    fn test_cli_parse_app_info() {
        let cli = Cli::try_parse_from(["app-dist", "app", "info", "some-app-id"]).unwrap();
        match cli.command {
            Commands::App(app::AppCommands::Info { app }) => {
                assert_eq!(app, "some-app-id");
            }
            _ => panic!("expected App Info command"),
        }
    }

    #[test]
    fn test_cli_parse_release() {
        let cli = Cli::try_parse_from([
            "app-dist",
            "release",
            "--app",
            "my-app",
            "--version",
            "1.2.3",
            "--build",
            "42",
            "--out-dir",
            "/tmp/dist",
            "--skip-notarize",
        ])
        .unwrap();
        match cli.command {
            Commands::Release(args) => {
                assert_eq!(args.app, Some("my-app".to_string()));
                assert_eq!(args.version, Some("1.2.3".to_string()));
                assert_eq!(args.build, Some(42));
                assert!(args.skip_notarize);
            }
            _ => panic!("expected Release command"),
        }
    }

    #[test]
    fn test_cli_parse_testers_add() {
        let cli = Cli::try_parse_from([
            "app-dist",
            "testers",
            "add",
            "--app",
            "my-app",
            "--email",
            "alice@example.com",
        ])
        .unwrap();
        match cli.command {
            Commands::Testers(testers::TesterCommands::Add(args)) => {
                assert_eq!(args.app, "my-app");
                assert_eq!(args.email, "alice@example.com");
            }
            _ => panic!("expected Testers Add command"),
        }
    }

    #[test]
    fn test_cli_parse_testers_list() {
        let cli = Cli::try_parse_from(["app-dist", "testers", "list", "--app", "my-app"]).unwrap();
        match cli.command {
            Commands::Testers(testers::TesterCommands::List { app }) => {
                assert_eq!(app, "my-app");
            }
            _ => panic!("expected Testers List command"),
        }
    }

    #[test]
    fn test_cli_parse_status() {
        let cli = Cli::try_parse_from(["app-dist", "status"]).unwrap();
        matches!(cli.command, Commands::Status);
    }

    #[test]
    fn test_cli_parse_api_url() {
        let cli = Cli::try_parse_from([
            "app-dist",
            "--api-url",
            "https://custom.example.com",
            "status",
        ])
        .unwrap();
        assert_eq!(cli.api_url, Some("https://custom.example.com".to_string()));
    }
}

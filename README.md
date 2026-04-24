# app-dist

> **Early development — not yet in production.** This CLI is actively being built and is not ready for use. The commands and API endpoints it targets are still under development. Star the repo or watch for releases if you're interested.

**Ship macOS betas with one CLI command.**

`app-dist` handles the full distribution pipeline for independent macOS developers: archive, sign, notarize, package, upload, and deliver — all from your terminal.

## Install

```bash
brew tap memetic-research-labs/tap
brew install app-dist
```

Or download the latest release from [GitHub Releases](https://github.com/memetic-research-labs/app-dist-cli/releases).

## Quick Start

```bash
# Authenticate
app-dist login

# Create your app
app-dist app create --name "MyApp" --bundle-id "com.example.myapp"

# Configure signing (interactive)
app-dist setup signing

# Ship a release
app-dist release --app <app-id> --version 1.0.0 --build 1

# Add testers
app-dist testers add --app <app-id> --email alice@example.com

# Send download links
app-dist testers notify --app <app-id>
```

## Commands

| Command | Description |
|---------|-------------|
| `app-dist login` | Authenticate and store API key in macOS Keychain |
| `app-dist whoami` | Show current authenticated developer info |
| `app-dist rotate-key` | Rotate your API key |
| `app-dist setup signing` | Configure Apple signing credentials interactively |
| `app-dist app create` | Register a new app on the platform |
| `app-dist app list` | List all your apps |
| `app-dist app info <id>` | Show app details |
| `app-dist release` | Build, sign, notarize, package, and upload a release |
| `app-dist testers add` | Add a tester to an app |
| `app-dist testers list` | List testers for an app |
| `app-dist testers notify` | Send download links to all testers |
| `app-dist status` | Dashboard overview |

### `app-dist release` Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--app` | from `app-dist.yml` | App ID |
| `--version` | from `Info.plist` | Version string |
| `--build` | from `Info.plist` | Build number |
| `--project` | from `app-dist.yml` | Xcode project path |
| `--scheme` | from `app-dist.yml` | Xcode scheme |
| `--out-dir` | `dist` | Output directory |
| `--skip-notarize` | `false` | Skip Apple notarization |
| `--skip-sign` | `false` | Skip code signing and export |

## Configuration

`app-dist` reads from `app-dist.yml` in your project root:

```yaml
app: <app-id>
scheme: MyApp
xcode_project: MyApp.xcodeproj
```

The API base URL can be set via:
- `--api-url` flag
- `APP_DIST_API_URL` environment variable
- Defaults to `https://api.app-dist.com`

## Security

- API keys are stored in macOS Keychain (service: `app-dist`, user: `api-key`)
- All platform communication is over HTTPS
- No plaintext secrets are written to disk or config files

## Build from Source

Requires Rust 1.56+ and macOS:

```bash
git clone https://github.com/memetic-research-labs/app-dist-cli.git
cd app-dist-cli
cargo build --release
./target/release/app-dist --help
```

## License

[MIT](LICENSE) — Memetic Research Laboratories LLC

# Configuring Your App

Set up your Xcode project to work with the app-dist CLI. This covers the `app-dist.yml` configuration file and the CLI's global settings.

---

## app-dist.yml

The CLI looks for a configuration file named `app-dist.yml` in your project root. This file tells the CLI which app to build, how to find your Xcode project, and where to upload releases.

### Create the file

Create `app-dist.yml` in the root of your Xcode project:

```yaml
app: <app-id>
scheme: MyApp
xcode_project: MyApp.xcodeproj
```

### Configuration Reference

| Field | Required | Description |
|-------|----------|-------------|
| `app` | Yes | Your app ID on the app-dist platform (obtained after `app-dist app create`) |
| `scheme` | Yes* | The Xcode scheme to build. Must match your `.xcodeproj` exactly |
| `xcode_project` | Yes* | Path to the `.xcodeproj` file, relative to the project root |
| `bundle_id` | No | Bundle identifier override (default: read from `Info.plist`) |
| `homepage_url` | No | App homepage URL (shown in the download page) |
| `support_email` | No | Support email address (shown in download emails) |

\* Required if not provided via CLI flags (`--scheme`, `--project`)

### Example

```yaml
app: abc123
scheme: Vapor
xcode_project: Vapor/Vapor.xcodeproj
bundle_id: lol.mrl.app.Vapor
homepage_url: https://vapor.app
support_email: support@vapor.app
```

### How the CLI resolves configuration

The CLI looks for values in this order:

1. CLI flags (`--scheme`, `--project`, `--app`)
2. `app-dist.yml` in the current directory
3. Environment variables (`APP_DIST_SCHEME`, `APP_DIST_PROJECT`, `APP_DIST_APP`)

---

## API Base URL

By default, the CLI talks to `https://api.app-dist.com`. You can point it to a different environment:

```bash
# Via flag
app-dist release --app abc123 --api-url https://staging.api.app-dist.com

# Via environment variable
export APP_DIST_API_URL=https://staging.api.app-dist.com
```

---

## Authentication

The CLI stores your API key in macOS Keychain (service: `app-dist`, user: `api-key`). It's never written to disk or config files.

### Login

```bash
app-dist login
```

Prompts for your API key (starts with `apd_`). Validates the key against the API before storing it.

### Check who you're authenticated as

```bash
app-dist whoami
```

### Rotate your API key

```bash
app-dist rotate-key
```

Generates a new key, invalidates the old one, and stores it in Keychain. You'll need to update any CI/CD configurations that reference the old key.

### Logout

```bash
app-dist logout
```

Removes the stored API key from Keychain.

---

## Signing Setup (Interactive)

The CLI includes an interactive setup wizard for configuring Apple signing credentials:

```bash
app-dist setup signing
```

This walks you through:
1. Detecting your Apple Team ID from installed certificates
2. Setting the keychain profile name for notarization
3. Generating an `app-dist.yml` with the detected values

You can also configure signing manually in `app-dist.yml` or via CLI flags:

```bash
app-dist release --scheme MyApp --project MyApp.xcodeproj
```

### Required Apple Credentials

| Credential | Purpose | How to Get |
|-----------|---------|-----------|
| Developer ID Application certificate | Code signing (Gatekeeper) | [developer.apple.com/account](https://developer.apple.com/account) |
| App Store Connect API key (.p8) | xcodebuild provisioning & notarization | [App Store Connect > Integrations](https://appstoreconnect.apple.com) |
| notarytool keychain profile | Apple notarization | `xcrun notarytool store-credentials` |

> **Note:** These credentials are stored on your Mac in Keychain or as files. The CLI never sends them to the app-dist platform.

---

## Project Root Detection

When you run commands from different directories, the CLI searches upward for `app-dist.yml`:

```
~/code/myapp/
├── app-dist.yml        ← CLI finds this
├── MyApp/
│   └── MyApp.xcodeproj
└── src/

# Running from anywhere inside:
cd ~/code/myapp && app-dist release
```

You can also run from outside the project by specifying the path:

```bash
cd ~ && app-dist release --project ~/code/myapp/MyApp.xcodeproj --scheme MyApp
```

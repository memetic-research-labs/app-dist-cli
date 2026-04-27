# Building & Distributing

The core workflow: build, sign, notarize, upload, and deliver — all from your terminal.

---

## Overview

```
app-dist release --app <app-id>
```

This single command handles the entire pipeline:

1. **Archive** — `xcodebuild archive` with your scheme and project
2. **Sign** — Developer ID export with hardened runtime
3. **Verify** — `codesign --verify --deep --strict`
4. **Notarize** — Submit to Apple, wait for acceptance, staple ticket
5. **DMG** — Create a compressed disk image
6. **Upload** — Upload DMG and ZIP to the platform
7. **Register** — Create a release record

---

## `app-dist release`

Build, sign, notarize, and upload a release in one command.

```bash
app-dist release --app <app-id>
```

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--app` | From `app-dist.yml` | App ID on the platform |
| `--version` | From `Info.plist` | Version string (e.g., `1.2.0`) |
| `--build` | From `Info.plist` | Build number |
| `--scheme` | From `app-dist.yml` | Xcode scheme |
| `--project` | From `app-dist.yml` | Path to `.xcodeproj` |
| `--out-dir` | `dist` | Output directory for DMG/ZIP |
| `--skip-notarize` | `false` | Skip Apple notarization (faster local builds) |
| `--skip-sign` | `false` | Skip code signing and export entirely |
| `--archive-path` | `build/<scheme>.xcarchive` | Use an existing archive instead of rebuilding |

### What it produces

| Output | Description |
|--------|-------------|
| `dist/<App>-<Version>-<Build>.dmg` | Signed, notarized disk image |
| `dist/<App>-<Version>-<Build>.zip` | Sparkle-compatible ZIP archive |
| `dist/<App>-<Version>-<Build>.sha256` | SHA-256 checksums |

### Example output

```
✓ Archived Vapor (1.2.0)
✓ Signed with Developer ID
✓ Notarized by Apple
✓ DMG created (42 MB)
✓ Uploaded to R2 edge
  Release ID: abc123
```

---

## Step-by-Step: First Release

### 1. Configure your project

```bash
cd /path/to/your/project
```

Create `app-dist.yml` in the project root:

```yaml
app: <app-id>
scheme: MyApp
xcode_project: MyApp.xcodeproj
```

### 2. Authenticate

```bash
app-dist login
```

### 3. Create your app on the platform

```bash
app-dist app create --name "MyApp" --bundle-id "com.example.myapp"
```

Save the returned app ID — you'll use it in `app-dist.yml`.

### 4. Ship

```bash
app-dist release
```

That's it. The CLI handles everything else automatically.

---

## Sending to Testers

After uploading a release, send download links to your testers:

```bash
app-dist testers add --app <app-id> --email alice@example.com
app-dist testers add --app <app-id> --email bob@example.com
app-dist testers notify --app <app-id>
```

`notify` emails each tester a unique, token-gated download link with configurable attempt limits and expiry.

---

## Troubleshooting

### "No --app specified and no app-dist.yml found"

The CLI can't determine which app to build. Either:
- Create `app-dist.yml` in your project root
- Pass `--app <id>` explicitly

### "scheme and project are required"

The CLI needs to know how to build your project. Set them in `app-dist.yml` or pass flags:
```bash
app-dist release --scheme MyApp --project MyApp/MyApp.xcodeproj
```

### "Build number is required"

Set `--build` explicitly or ensure your `Info.plist` has a `CFBundleVersion` value:
```bash
app-dist release --build 1
```

### "Notarization failed"

Common causes:
- **Expired API key** — regenerate from App Store Connect
- **Network issues** — Apple's notary service is occasionally unavailable; retry
- **Unsigned or ad-hoc signed app** — ensure Developer ID signing is used, not local/ad-hoc

Use `--skip-notarize` for fast local builds where notarization isn't needed.

### "xcodebuild archive failed"

- Verify the scheme and project path are correct
- Ensure the project builds successfully in Xcode first
- Check that your Developer ID certificate is installed in Keychain

### "codesign verification failed"

Common causes:
- **Missing hardened runtime entitlement** — add "Hardened Runtime" to your target's capabilities
- **Unsigned framework** — ensure all embedded frameworks are signed
- **Wrong signing identity** — the CLI uses Developer ID, not App Store

---

## Makefile Integration (Advanced)

If you prefer `make` commands, the CLI can be called from a Makefile:

```makefile
release:
	app-dist release --app $(APP_ID) --scheme $(SCHEME) --project $(PROJECT)

grants:
	app-dist testers add --app $(APP_ID) --email alice@example.com
	app-dist testers notify --app $(APP_ID)
```

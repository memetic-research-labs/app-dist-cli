# Getting Started

Everything you need to set up before distributing your first macOS app with app-dist.

## What app-dist Does

app-dist handles the full distribution pipeline for independent macOS developers: build, sign, notarize, upload, and deliver — all from your terminal. One CLI command sends a signed DMG to every tester.

---

## Collateral Checklist

You will need to provide or configure the items below. Don't worry if some terms are unfamiliar — each one is explained in detail below.

| # | What | Where It's Used | Cost |
|---|------|--------------|------|
| 1 | Apple Developer Program membership | Signing & notarization | $99/year |
| 2 | Developer ID Application certificate | Code signing | Included |
| 3 | App Store Connect API key (.p8 file) | Provisioning & notarization | Included |
| 4 | notarytool keychain profile | Notarization | Included |
| 5 | Cloudflare account | Hosting (R2, D1, Worker) | Free tier works |
| 6 | Wrangler CLI | Cloudflare ops | Free |
| 7 | R2 storage bucket | DMG file hosting | Free tier: 10 GB |
| 8 | D1 database | Grant tracking | Free tier: 5 GB |
| 9 | Resend account + API key | Sending download emails | Free: 3K/mo |
| 10 | Verified sending domain (Resend) | Email deliverability | Free |
| 11 | Xcode with macOS | Building your app | Free (macOS) |

---

## Step 1: Apple Developer Program

You need a paid Apple Developer Program membership to distribute apps outside the App Store. This gives you access to Developer ID certificates and the notarization system.

1. Sign up at [developer.apple.com/programs/enroll/](https://developer.apple.com/programs/enroll/)
2. Complete enrollment (personal or organization)
3. Note your **Team ID** — a 10-character string like `YRQLJYMX5S` found on your Membership page

## Step 2: Developer ID Application Certificate

This certificate lets your users install your app without Gatekeeper warnings. It identifies you as a known developer.

### Get the certificate

1. Sign in to [developer.apple.com/account](https://developer.apple.com/account)
2. Go to **Certificates, Identifiers & Profiles** > **Certificates**
3. Click **+** > **Developer ID Application**
4. Create a Certificate Signing Request (CSR):
   - Open **Keychain Access** > **Certificate Assistant** > **Request a Certificate From a Certificate Authority**
   - Enter your email, select **Saved to disk**, click **Continue**
   - Upload the CSR to Apple
   - Download the certificate and double-click to install it in Keychain

### Verify

```bash
security find-identity -v -p codesigning | grep "Developer ID"
```

You should see your name and certificate. If nothing appears, the cert isn't installed correctly.

## Step 3: App Store Connect API Key

This key lets xcodebuild authenticate with Apple's servers for provisioning and notarization — no Xcode login prompts during automated builds.

### Generate the key

1. Go to [App Store Connect](https://appstoreconnect.apple.com) > **Users and Access** > **Integrations** > **App Store Connect API**
2. Click **+** (or **Generate API Key**)
3. Name it something descriptive (e.g., "app-dist CI")
4. Select the **Developer** role
5. Click **Generate**
6. **Download the .p8 file** — you only get one chance. Save it to a secure location:
   ```bash
   mkdir -p ~/.keys
   mv ~/Downloads/AuthKey_XXXXXXXXXX.p8 ~/.keys/AuthKey_XXXXXXXXXX.p8
   chmod 600 ~/.keys/AuthKey_XXXXXXXXXX.p8
   ```
7. Note the **Key ID** (e.g., `9D3T93T6PP`)
8. Note the **Issuer ID** (e.g., `bd5a8547-b7a5-4ec8-82d2-0c22eeaec185`)

### What you'll need later

| Value | Env Var Name | Example |
|-------|-------------|---------|
| Path to .p8 file | `AUTH_KEY_PATH` | `~/.keys/AuthKey_9D3T93T6PP.p8` |
| Key ID | `AUTH_KEY_ID` | `9D3T93T6PP` |
| Issuer ID | `AUTH_ISSUER_ID` | `bd5a8547-b7a5-4ec8-82d2-0c22eeaec185` |
| Team ID | `TEAM_ID` | `YRQLJYMX5S` |

## Step 4: notarytool Keychain Profile

notarytool needs credentials to submit DMGs to Apple for notarization. Storing them in a keychain profile lets the build run non-interactively.

### Create the profile (recommended: using .p8 key)

```bash
xcrun notarytool store-credentials "mrl-notary" \
  --key ~/.keys/AuthKey_XXXXXXXXXX.p8 \
  --key-id YOUR_KEY_ID \
  --issuer YOUR_ISSUER_ID
```

You can name the profile anything. We use `mrl-notary` in examples — update it to match your `.env`.

### Alternative: Apple ID + app-specific password

```bash
xcrun notarytool store-credentials "mrl-notary" \
  --apple-id "your@email.com" \
  --team-id "YOUR_TEAM_ID" \
  --password "xxxx-xxxx-xxxx-xxxx"
```

App-specific passwords are created at [appleid.apple.com](https://appleid.apple.com) > **Sign-In and Security** > **App-Specific Passwords**.

### Verify

```bash
xcrun notarytool history --keychain-profile "mrl-notary"
```

You should see a table of recent submissions (empty is fine — it means no notarizations yet).

## Step 5: Cloudflare Account

app-dist uses Cloudflare's free-tier infrastructure (Workers, R2, D1). No credit card required.

### Install and authenticate Wrangler

```bash
npm install -g wrangler
wrangler login
```

This opens a browser for OAuth. Wrangler stores the token locally.

### Create an R2 bucket

R2 stores your signed DMG files. Create one named after your app:

```bash
wrangler r2 bucket create <app>-downloads
# Example: wrangler r2 bucket create vapor-downloads
```

### Create a D1 database

D1 stores download grants, attempts, and developer accounts.

```bash
wrangler d1 create <app>-downloads
```

**Save the `database_id` from the output** — you'll need it in your `wrangler.toml`.

### Generate secrets

Two secrets are required for the Worker. Generate them:

```bash
openssl rand -base64 32
# Use this value for both ADMIN_API_TOKEN and EMAIL_HMAC_SECRET
```

Then set them as Wrangler secrets:

```bash
wrangler secret put ADMIN_API_TOKEN "your-generated-token" -c data/<app>/wrangler.toml
wrangler secret put EMAIL_HMAC_SECRET "your-generated-token" -c data/<app>/wrangler.toml
```

> **Security note**: `ADMIN_API_TOKEN` authenticates admin API calls (creating/revoking grants). `EMAIL_HMAC_SECRET` is the key used to hash email addresses and IP addresses before storage. Both should be strong, unique, and never committed to version control.

## Step 6: Resend Account (Email)

app-dist sends download links to testers via [Resend](https://resend.com).

### Set up

1. Sign up at [resend.com](https://resend.com)
2. Go to **API Keys** and create a key
3. Copy the API key (starts with `re_`)

### Verify your sending domain

In Resend, go to **Domains** > **Add Domain** and verify the domain you want emails sent from (e.g., `no-reply@app-dist.com`). This typically requires adding a DNS TXT or MX record.

> **What about app-dist.com?** If you're using the hosted platform, the sending domain is pre-configured. If you're self-hosting, you must verify your own domain in Resend.

### What you'll need later

| Value | Env Var Name | Example |
|-------|-------------|---------|
| API key | `RESEND_API_KEY` | `re_dMZ6wsZT_...` |
| From address | `EMAIL_FROM` | `no-reply@app-dist.com` |
| Subject line | `EMAIL_SUBJECT` | `Your MyApp beta download link` |

## Step 7: Install the CLI

The app-dist CLI is open source (MIT license) and distributed as a single Rust binary with no runtime dependencies.

### Install via Homebrew (recommended)

```bash
brew tap memetic-research-labs/tap
brew install app-dist
```

### Or download from GitHub Releases

```bash
# Download the latest universal binary from
# https://github.com/memetic-research-labs/app-dist-cli/releases
```

### Verify

```bash
app-dist --version
```

## Step 8: Verify Everything

Run the built-in prerequisite checker:

```bash
make check APP=<app>
```

This verifies:
- `xcodebuild` is available
- `xcrun`, `hdiutil`, `codesign`, `ditto` are installed
- `node` and `npx` are available (with versions)
- `wrangler` is installed
- Your Developer ID certificate is in Keychain
- Your notarytool profile is configured
- Your app's `.env` and `wrangler.toml` exist

If anything is marked **MISSING**, set it up using the steps above before proceeding.

## What the Platform Provides vs. What You Provide

| Provided by app-dist | You Provide |
|---------------------|-------------|
| R2 object storage (hosts DMGs) | macOS hardware + Xcode |
| D1 database (stores grants) | Apple Developer Program membership |
| Cloudflare Worker (serves downloads) | Xcode project with signing entitlements |
| Token generation and validation | Developer ID Application certificate |
| Email delivery via Resend | App Store Connect API key |
| Download attempt tracking | notarytool credentials |
| Privacy (HMAC'd emails, hashed IPs) | Wrangler authentication |
| CLI tool (open source) | Resend API key + verified domain |
| | Your app's source code and testers |

## Next Steps

Once everything is verified, continue to [Configuring Your App](configuring-your-app.html) to set up your app's configuration files.

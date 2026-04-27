# Managing Testers & Download Grants

How beta testers interact with your app's download links — tokens, expiry, revocation, and privacy.

---

## How Downloads Work

Every download link is a unique, single-use URL like:

```
https://app-dist.com/d/1w_Tkj4FRrIK7XKDKRlpkuaO3bpd2t1yaw7OGDCJ8lY
```

These are **not** public URLs. Each token is:

- **Opaque** — 32 random bytes, unpredictable
- **Time-limited** — expires after a configurable period
- **Attempt-limited** — max downloads per token (default 3, up to 20)
- **Revocable** — instantly disabled

### The Flow

1. You add a tester's email: `app-dist testers add`
2. You send download links: `app-dist testers notify`
3. The tester clicks their link
4. The platform validates the token, checks expiry and attempts, and streams the DMG
5. The tester downloads and opens the app

---

## Adding Testers

```bash
app-dist testers add --app <app-id> --email alice@example.com
app-dist add --app <app-id> --email bob@example.com
```

### List your testers

```bash
app-dist testers list --app <app-id>
```

---

## Sending Download Links

```bash
app-dist testers notify --app <app-id>
```

This emails every tester on your list a unique download link. Each email contains:

- A single-use download URL
- The app name and version
- Expiry information
- Your support email address

### What the tester sees

1. Tester receives the email
2. Clicks the download link
3. Browser downloads the DMG
4. Double-clicks to mount and install (signed + notarized, so Gatekeeper is happy)

---

## Token Configuration

When you create grants (via `notify` or the admin API), you can configure:

| Setting | Range | Default | Description |
|---------|-------|---------|-------------|
| `max_attempts` | 1–20 | 3 | How many times the link can be downloaded |
| `expires_hours` | 1–168 | 72 | How long before the link expires (168 = 7 days) |

### Why tokens expire

Expiring tokens keep your beta builds secure. If a download link leaks or is shared unintentionally, it becomes useless after the expiry window.

### Why tokens are attempt-limited

A link shared with 3 attempts prevents a single link from being distributed widely. If a tester needs more attempts, re-issue them a new grant.

---

## Revoking Access

### Revoke a specific tester

If you need to remove access for a specific person:

```bash
app-dist testers revoke --app <app-id> --email alice@example.com
```

This immediately invalidates all active download links for that email.

### Revoke a specific download link

If you know the grant/token:

```bash
app-dist testers revoke --app <app-id> --token <token>
```

### Revoke all active links

Contact support@app-dist.com to revoke all active links for an app at once.

---

## Privacy: What Gets Stored

app-dist is designed to minimize the data it holds about your testers and downloaders.

| Data | How It's Stored |
|------|---------------|
| **Email addresses** | HMAC-SHA256 digest. The plaintext email is never written to the database. It exists only in transit during grant creation. The HMAC digest cannot be reversed to recover the email. |
| **Download tokens** | SHA-256 hash only. The raw token is generated at grant creation, returned once, and never stored. The database only holds the hash. |
| **IP addresses** | HMAC-SHA256 hash. The raw IP is hashed with a server-side secret before logging. The original IP is never stored. |
| **User-agent strings** | HMAC-SHA256 hash. Same treatment as IP addresses. |
| **Download attempts** | Logged with grant ID, result (`served`, `expired`, `revoked`, etc.), and the hashed IP/UA — no plaintext. |

### What this means in practice

- If the database is breached, email addresses cannot be recovered
- Download tokens cannot be forged — they're single-use and hashed
- There is no way to associate a download with a specific person from the database alone
- Your testers' privacy is protected by design, not by policy

### What you (the developer) should know

- The `grants.csv` file in your project is the only place plaintext emails exist at rest. Keep it secure.
- Once a grant is created, the platform cannot recover the tester's email to re-send — you'll need to add them again with a new grant.
- `app-dist testers list` can only show hashed identifiers, not email addresses, for already-issued grants. Keep your own record of who you've invited.

---

## Re-issuing Grants

If a tester's link has expired or been consumed, issue them a new grant:

```bash
app-dist testers add --app <app-id> --email alice@example.com
app-dist testers notify --app <app-id>
```

No need to remove the old grant — the new one is independent. Old expired grants simply become inactive.

If you're issuing a new build and want everyone to get fresh links:

1. Upload the new build: `app-dist release`
2. Re-send to everyone: `app-dist testers notify --app <app-id>`

The `notify` command sends to all testers on the list, regardless of whether they have active grants.

---

## Download Attempt Tracking

Every download attempt is logged with:

| Field | Value |
|-------|-------|
| Grant ID | Which download grant was used |
| Result | `served`, `expired`, `revoked`, `attempt_limit`, `not_found`, `auth_failed`, `asset_missing` |
| Timestamp | When the attempt occurred |
| IP hash | HMAC of the requester's IP |
| UA hash | HMAC of the requester's browser |

You can query this data via the admin API or the platform dashboard (coming soon) to understand:
- How many testers actually downloaded your build
- Which links expired without being used
- Whether any links were revoked or hit attempt limits

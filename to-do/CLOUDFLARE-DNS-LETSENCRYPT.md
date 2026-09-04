# Cloudflare DNS + per-domain SSL providers (CPN)

Author: master3395. Spellings: **Cloudflare**, **Let's Encrypt**, **ZeroSSL**. Zero CyberPanel branding.

## Schema (per domain / subdomain)

Stored on each site JSON under `/var/lib/cpn/sites/<domain>.json` (`schema_version` 2):

```json
{
  "ssl": {
    "provider": "letsencrypt",
    "include_subdomains_on_cert": false,
    "shared_cert_owner": null,
    "last_issue_unix": 0,
    "last_error": "",
    "custom_cert_path": null,
    "custom_key_path": null,
    "install_origin_cert": true
  }
}
```

| `provider` | Behavior |
|------------|----------|
| `letsencrypt` | Auto issue/renew via certbot (webroot or Cloudflare DNS-01) |
| `zerossl` | Auto ACME via ZeroSSL directory; EAB in `/var/lib/cpn/ssl/zerossl-eab.json` (not in repo) |
| `cloudflare_ca` | Uses Cloudflare API token; DNS-01 origin material when plugin present; Origin CA CSR API still partial |
| `custom` | User upload only; **no** auto-renew |
| `none` | Do not issue, install, or auto-renew |

There is **no** account-level SSL switch that rewrites all domains. New-site default only: `/var/lib/cpn/ssl-defaults.json`.

Custom PEMs: `/var/lib/cpn/ssl/<domain>/fullchain.pem` + `privkey.pem` (key mode 600).

Cloudflare DNS token: `/var/lib/cpn/cloudflare.json` (mode 600, masked in UI).

## SAN / shared certs

- Apex (or cert owner) may set `include_subdomains_on_cert=true`.
- Members = owner + children whose **provider matches** the owner and are auto ACME.
- If a subdomain switches to Custom or None, it leaves the shared cert (`shared_cert_owner` cleared) and gets its own path.
- Limitations: Let's Encrypt / ZeroSSL rate limits; Cloudflare Origin CA host/SAN constraints; wildcard needs DNS-01.

## CLI

```text
cpn site create --domain example.com --owner admin --ssl-provider letsencrypt
cpn site create --domain blog.example.com --owner admin
  # inherits parent provider as initial value only
cpn site modify --domain blog.example.com --ssl-provider none
cpn site list
  # prints ssl= column
```

Values: `letsencrypt|zerossl|cloudflare_ca|custom|none`.

Installer/panel new-site default: Manage SSL "Default SSL provider for new sites" or `ssl-defaults.json`.

## Routes

| Path | Purpose |
|------|---------|
| `/dns/cloudflare` | Cloudflare DNS (Manage + API Settings) |
| `/security/ssl` | Per-domain providers, bulk issue, new-site default |
| `/security/ssl/provider` (POST) | Set one domain's provider (+ SAN opt-in) |
| `/security/ssl/issue` (POST) | Issue/renew for that domain's provider |
| `/security/ssl/upload` (POST) | Custom PEM upload |
| `/security/ssl/defaults` (POST) | New-site default only |
| Manage → SSL tab | Same controls scoped to one site |

## Policy notes

- Domains set to **None** or **Custom** are never forced onto Let's Encrypt.
- When provider is LE / Cloudflare CA and Cloudflare proxy is used, `install_origin_cert` documents origin material for the local stack.
- Honest errors if certbot missing, ZeroSSL EAB missing, or rate-limited.

## Browser reference (optional UX)

CyberPanel Cloudflare DNS page used as layout reference only (API Settings + Manage DNS + proxy toggles). No secrets copied into the repo.

## AL9 smoke

AlmaLinux 9 only (`127.0.0.1:2222`, `http://127.0.0.1:2087`):

1. `/dns/cloudflare?tab=api` loads; token masked after save.
2. `/security/ssl` lists sites with provider badges; set `cpn-lab-test.example` to None vs Let's Encrypt independently.
3. `cpn site create --help` shows `--ssl-provider`.
4. Issue LE on a lab domain without public DNS must show an honest certbot/DNS error (never fake success).

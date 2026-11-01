# Remote notification broadcast (News Targeted)

CPN accepts a **signed** push from `api.newstargeted.com` so operators can broadcast to all panel accounts (or a username list) without exposing a public unauthenticated inbox writer.

## Endpoint

`POST /api/nt/notifications/broadcast`

Unauthenticated by session. Requires HMAC headers. Disabled until config exists and `enabled` is true.

### Headers

| Header | Meaning |
|--------|---------|
| `X-CPN-Timestamp` | Unix seconds |
| `X-CPN-Nonce` | Unique id (alphanumeric, `-`, `_`) |
| `X-CPN-Signature` | `hex(hmac_sha256(secret, "{ts}\\n{nonce}\\n" + sha256_hex(raw_body)))` |

### Body (JSON)

```json
{
  "title": "Maintenance",
  "body": "Optional plaintext when AES is off",
  "category": "ops",
  "all_users": true,
  "usernames": ["cpnowner"],
  "operator": "discord-username",
  "body_enc": "",
  "body_nonce": ""
}
```

When `aes_key_b64` is set in config, `body_enc` + `body_nonce` (Base64) are required and `body` may be empty.

## Config file

Path: `$CPN_DATA_DIR/notifications/remote-push.json` (default `/var/lib/cpn/notifications/remote-push.json`), mode `600`.

```json
{
  "enabled": true,
  "hmac_secret": "<32+ char secret shared with api config>",
  "aes_key_b64": "",
  "max_skew_secs": 300,
  "rate_limit_per_minute": 30,
  "allowed_source_ips": []
}
```

Replay nonces: `notifications/remote-nonces.json`  
Rate buckets: `notifications/remote-rate.json`

## Existing APIs (unchanged)

- `GET /api/panel/notifications`
- `POST /api/panel/notifications/mark-read`
- `POST /api/panel/notifications` (session self-push)

## Audit

Successful and failed remote attempts append `panel.notifications.remote` to `panel-actions.jsonl`.

## Phone-home (register this install)

So News Targeted can discover panels on the internet (not only a manual URL list), CPN can register with the API.

### CLI

```bash
sudo cpn network set-public-url --url https://panel.example
# HMAC: env CPN_PHONEHOME_HMAC_SECRET, /var/lib/cpn/phonehome/config.json hmac_secret,
# build-time CPN_PHONEHOME_BUILD_SECRET, or existing remote-push.json hmac_secret
sudo cpn network phone-home --now
sudo cpn network phone-home --status
sudo cpn network phone-home --off   # opt out
```

### Behaviour

- Default **enabled** locally; worker runs shortly after panel start and about once per day.
- Never sends passwords. Payload: install_id, panel_public_url, version, hostname, optional contact_email.
- HTTPS public URL required unless `allow_http` is set in phonehome config (labs).
- On first successful register, API may return one-time `push_hmac`; CPN writes `notifications/remote-push.json` with `enabled: true`.

Operator UI: `https://cpn.newstargeted.com/installs` and compose checkbox for all phoned-home installs.

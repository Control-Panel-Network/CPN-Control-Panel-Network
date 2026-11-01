# CPN phone-home client

Date: 01/11/2026

## Files

| Path | Role |
|------|------|
| `src/panel_phonehome.rs` | Install id, config, HMAC register via curl, daily worker |
| `src/cli_network.rs` | `cpn network phone-home` |
| `src/main.rs` | Spawns background worker |
| `src/panel_notifications_remote.rs` | `enable_remote_push_with_hmac` after register |

## Secrets (never in git)

| Source | Use |
|--------|-----|
| `CPN_PHONEHOME_HMAC_SECRET` env | Preferred runtime |
| `/var/lib/cpn/phonehome/config.json` `hmac_secret` | Local override |
| `CPN_PHONEHOME_BUILD_SECRET` compile env | Official NT release pipeline only |
| `/var/lib/cpn/notifications/remote-push.json` | Fallback if already configured |

API side: `CPN_PHONEHOME_HMAC_SECRET` in `api.newstargeted.com` `config.php`.

## Fresh install flow

1. Set HTTPS `panel_public_url`.
2. Provide phone-home HMAC (env or config).
3. Upgrade/start panel (worker) or `cpn network phone-home --now`.
4. API stores install; returns `push_hmac` once; panel enables remote-push receiver.
5. Operator Send on cpn.newstargeted.com with "All phoned-home installs" fans out signed POSTs.

# MFA / 2FA / TOTP / passkey CLI reference

Operator reference for `cpn totp`, `cpn passkey`, and `cpn mfa`, plus account commands that affect MFA enrollment. For hostname / RP ID behavior see [MULTI-HOSTNAME-MFA.md](MULTI-HOSTNAME-MFA.md). For the rest of the CLI see [CLI.md](CLI.md).

> [!NOTE]
> Run `cpn totp --help`, `cpn passkey --help`, and `cpn mfa --help` on the installed binary for the authoritative syntax of that release.

## Requirements (all commands below)

| Rule | Detail |
|---|---|
| Root | Every `totp`, `passkey`, and `mfa` subcommand calls `require_root` (use `sudo cpn …`). Lab override: `CPN_ALLOW_NONROOT=1` with a non-production data dir. |
| Username | Always `--username <NAME>`. Resolution is case-insensitive against the panel account store (`shadows` and `Shadows` resolve to the same account). |
| Secrets on stdout | TOTP base32 secrets, backup codes, and raw enroll tokens are **not** printed on stdout. Enroll material goes to mode-600 temp files (`totp begin` / `totp confirm` / `mfa enroll-url`). `mfa enroll-url` prints `enroll_url_file=` (path only); treat the file contents as a secret capability. |
| Confirmations | Destructive clears prompt for `yes`/`y` (case-insensitive) unless `--yes` is passed. `no`/`n` or empty aborts. |

Quick discovery:

```bash
sudo cpn totp --help
sudo cpn passkey --help
sudo cpn mfa --help
sudo cpn mfa status --username shadows
```

---

## Command map

| Command | Purpose | Changes state? |
|---|---|---|
| `cpn mfa status` | Combined TOTP + passkey + enroll flags | No |
| `cpn mfa enroll-url` | Mint single-use HTTPS link for browser MFA / Security | Yes (pending enroll token) |
| `cpn mfa clear` | Clear TOTP + all passkeys + pending WebAuthn ceremonies | Yes |
| `cpn totp status` | TOTP enabled / pending enroll flags | No |
| `cpn totp begin` | Start TOTP enroll; write secret to mode-600 temp file | Yes (pending TOTP) |
| `cpn totp confirm` | Finish TOTP enroll with 6-digit `--code` | Yes (enables TOTP; backup codes file) |
| `cpn totp disable` | Remove TOTP secret and pending enroll | Yes |
| `cpn totp clear` | Alias of `totp disable` | Yes |
| `cpn passkey list` | List passkey id / label / type / timestamps | No |
| `cpn passkey rename` | Change passkey label by `--id` | Yes (label only) |
| `cpn passkey remove` | Delete one passkey by `--id` | Yes |
| `cpn passkey clear` | Delete all passkeys for the account | Yes |

---

## `cpn mfa`

Combined MFA status, browser enroll link, and full clear.

```bash
sudo cpn mfa status --username shadows
sudo cpn mfa enroll-url --username shadows
sudo cpn mfa clear --username shadows --yes
```

### `mfa status`

**Flags:** `--username` (required).

**Stdout (tab-separated flags, no secrets; username omitted):**

- `totp_enabled=` / `totp_pending_enroll=`
- `passkey_count=`
- `totp_required=` (bootstrap preference that MFA must be enrolled)
- `needs_mfa_enrollment=`
- `cli_enroll_token_pending=` (unused `enroll-url` token still valid)

### `mfa enroll-url`

**Flags:** `--username` (required).

**Effect:** Creates a short-lived, single-use enroll token for that username (replaces any prior unused token for the same user). Writes the HTTPS enroll URL to a mode-600 temp file and prints:

- `enroll_url_file=` (path to the mode-600 file; contents are `# expires_at_unix=` / `# expires_in_secs=` comments plus the primary-panel HTTPS URL)
- `expires_in_secs=` (**900** = 15 minutes; absolute `expires_at_unix` is only inside the file)
- operator `note=` about desktop browser / HTTPS / RP ID
- optional `warning=` when `panel_public_url` is unset

Username and the raw URL/token are not printed on stdout (CodeQL cleartext-logging).

**Fails when:** account is disabled (enable first with `sudo cpn account enable --username shadows --yes`).

**What it does not do:** It does not register a passkey or enable TOTP by itself. The administrator must open the URL in a desktop browser and complete enrollment in the UI.

#### Safe operator usage (enroll links)

1. On the server: `sudo cpn mfa enroll-url --username shadows`.
2. Read `enroll_url_file=` (for example `sudo cat` the path) and copy the URL.
3. Open it on a **desktop** browser by pasting into the **address bar** (or open from the same SSH/client path that does not fetch URLs for previews).
4. Complete **Continue to Security** (or the confirm step your build shows), then enroll passkey and/or TOTP in the panel.
5. Verify: `sudo cpn passkey list --username shadows` and/or `sudo cpn totp status --username shadows`.

| Rule | Detail |
|---|---|
| TTL | 15 minutes (`MFA_ENROLL_TOKEN_TTL_SECS` / `expires_in_secs=`) |
| Scope | Always primary panel hostname / public URL (not host aliases) |
| Pending | One unused link per username; a new `enroll-url` replaces the previous unused link |
| Do not preview | Do **not** paste the URL into chat, Discord, email, or other clients that auto-fetch links first. Prefer the desktop address bar. |
| CPN 1.4.11 and older | GET/HEAD of the enroll URL **burns** the single-use token. A preview fetch makes the real open fail. |
| Intended / fixed builds | GET/HEAD only **peeks** and shows a confirm page; the token is consumed on **POST** (Continue). Still avoid previews: treat the URL as sensitive and open it yourself. |
| Already enrolled | After continue, lands on MFA enroll or **Modify User → Security** so you can add **additional** passkeys without clearing TOTP. |
| Add more passkeys | Run `enroll-url` again (or sign in normally) and register another passkey on the Security tab. TOTP remains one secret per account. |

### `mfa clear`

**Flags:** `--username` (required), `--yes` (optional skip prompt).

**Effect:**

1. Force-clears TOTP (enabled secret and pending enroll) for that account.
2. Removes **all** passkeys for that account.
3. Deletes pending WebAuthn ceremony JSON files under the install ceremony dir (login/register challenges for the panel; count returned as `ceremonies_removed=`).

**Stdout example shape:** `cleared mfa ok	totp_was_enabled=…	passkeys_removed=…	ceremonies_removed=…`

**Afterward:** For accounts with `totp_required`, the next password login may land on `/account/security/enroll-2fa` until MFA is enrolled again. That is expected.

---

## `cpn totp`

TOTP authenticator for a panel account. Does not touch passkeys.

```bash
sudo cpn totp status --username shadows
sudo cpn totp begin --username shadows
# read totp_enroll_file= (mode 600): base32 secret + otpauth URI
sudo cpn totp confirm --username shadows --code 123456
# read backup_codes_file= (mode 600); store offline, then delete the temp file
sudo cpn totp disable --username shadows --yes
sudo cpn totp clear --username shadows --yes   # same as disable
```

| Subcommand | Required flags | Optional | State change |
|---|---|---|---|
| `status` | `--username` | | None. Prints `totp_enabled=` and `pending_enroll=`. |
| `begin` | `--username` | | Starts pending enroll. Writes secret material to `totp_enroll_file=` (mode 600). Stdout: `totp_enroll_pending=true`, path, and a note to run `confirm`. |
| `confirm` | `--username`, `--code` | | Completes pending enroll if `--code` matches. Writes backup codes to `backup_codes_file=` (mode 600). Stdout: `totp_enabled=true`, path, `backup_codes_count=`. |
| `disable` | `--username` | `--yes` | Removes TOTP secret and pending enroll **without** an authenticator code. |
| `clear` | `--username` | `--yes` | **Alias of `disable`** (same confirmation and same clear). |

Notes:

- Prefer `begin` / `confirm` over pasting secrets into shell history. Delete or secure temp files after enrollment.
- `disable` / `clear` stop authenticator codes for that account. Passkeys are left alone; use `passkey clear` or `mfa clear` if those must go too.
- Replacing an already-enabled TOTP: clear or disable first (or use the panel Security UI), then `begin` / `confirm` again.

---

## `cpn passkey`

WebAuthn passkeys for a panel account. List / rename / remove / clear only. **Registration cannot finish over SSH**; use `mfa enroll-url` (or the Security tab while signed in).

```bash
sudo cpn passkey list --username shadows
sudo cpn passkey rename --username shadows --id <credential-id> --label "Laptop Windows Hello"
sudo cpn passkey remove --username shadows --id <credential-id>
sudo cpn passkey clear --username shadows --yes
```

| Subcommand | Required flags | Optional | State change |
|---|---|---|---|
| `list` | `--username` | | None. One line per key: `id=`, `label=`, `type=`, `created=`, `last_used=` (human timestamps, or `-`). Empty store prints `(no passkeys)`. |
| `rename` | `--username`, `--id`, `--label` | | Updates the label only. `--id` comes from `list` (not secret material). |
| `remove` | `--username`, `--id` | | Deletes that credential. |
| `clear` | `--username` | `--yes` | Deletes **all** passkeys for the account. Does **not** clear TOTP. |

Browser registration notes (after `enroll-url` or normal login):

- Platform authenticators (for example Windows Hello) are tried first; cross-platform security keys are the fallback.
- Passkeys are bound to the HTTPS hostname (RP ID). Enroll on the primary hostname for day-to-day use. See [MULTI-HOSTNAME-MFA.md](MULTI-HOSTNAME-MFA.md).

---

## Related account / password commands (MFA impact)

These are not under `totp` / `passkey` / `mfa`, but they change whether enrollment and login work.

| Command | MFA-related effect |
|---|---|
| `sudo cpn password --username shadows` | Resets the panel password only. **Does not** clear TOTP or passkeys. |
| `sudo cpn account passwd --username shadows …` | Same password reset path as `cpn password`. |
| `sudo cpn account deactivate --username shadows --yes` | Blocks sign-in and rejects sessions. `mfa enroll-url` fails until the account is enabled again. |
| `sudo cpn account enable --username shadows --yes` | Restores login so enroll links and MFA login work again. |
| `sudo cpn account delete --username shadows --yes` | Removes the account (and its MFA material with the account). |
| `sudo cpn account create …` | New accounts follow product rules for `totp_required` / must-change-password; enroll MFA after first login or via CLI. |

Password options that matter for safe resets: `--password-stdin`, `--generate` (writes a generated password to a mode-600 temp file; never put the new password on the argv line in shared logs).

```bash
sudo cpn password --username shadows --generate
sudo cpn account enable --username shadows --yes
sudo cpn mfa enroll-url --username shadows
```

---

## Operator playbooks

### Locked out of TOTP (still have SSH root)

```bash
sudo cpn totp status --username shadows
sudo cpn totp clear --username shadows --yes
# optional: also wipe passkeys
sudo cpn passkey clear --username shadows --yes
# or one shot:
sudo cpn mfa clear --username shadows --yes
```

Then sign in with password and re-enroll, or:

```bash
sudo cpn totp begin --username shadows
sudo cpn totp confirm --username shadows --code <from-authenticator>
# and/or
sudo cpn mfa enroll-url --username shadows
```

### Add a passkey / Windows Hello from SSH

```bash
sudo cpn mfa status --username shadows
sudo cpn mfa enroll-url --username shadows
# read enroll_url_file=; open URL on desktop address bar within 15 minutes; do not paste into chat first
sudo cpn passkey list --username shadows
```

### Inspect before changing anything

```bash
sudo cpn mfa status --username shadows
sudo cpn totp status --username shadows
sudo cpn passkey list --username shadows
```

---

## Build availability

| Capability | Notes |
|---|---|
| `totp status` / `disable` / `clear`, `mfa clear`, `passkey list` / `remove` / `clear` | Present on older stable packages that already shipped MFA CLI clears. |
| `mfa status`, `mfa enroll-url`, `totp begin`, `totp confirm`, `passkey rename` | Shipped with the MFA SSH enroll CLI line (`c77438e` and newer). Confirm with `cpn mfa --help` / `cpn passkey --help` on the host. |
| Enroll-url peek on GET/HEAD | Intended safe behavior on builds that include the enroll-cli confirm POST. CPN **1.4.11** and older burn the token on first GET/HEAD. |

---

## See also

- [CLI.md](CLI.md) : full operator CLI
- [MULTI-HOSTNAME-MFA.md](MULTI-HOSTNAME-MFA.md) : passkeys vs host aliases / RP ID
- [USER-SECURITY.md](USER-SECURITY.md) : administrator MFA expectations

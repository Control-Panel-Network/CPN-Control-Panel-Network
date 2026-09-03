# OneDrive sync and CPN lab sources

Pause OneDrive sync for `CPN-Control-Panel-Network` while editing installer Rust/UI.

OneDrive repeatedly reverted local `src/` during AL9 lab work. Canonical working tree for verified lab fixes was on the guest at `/home/cpn/cpn-build-v02/`. Prefer committing from a non-OneDrive clone (for example under `%LOCALAPPDATA%\Temp`) or from the guest when sync fights local edits.

Lab bootstrap username `Ådmin` with email `admin@example.com` is user-entered account-setup data, not a code default and not an encoding bug.

## Summary
- Adds CPN-branded sidebar sections (Hosting / Account / Administration) and tile hub pages for Server, Email, Databases & FTP, and Backups.
- Every listed tile has a route and page shell; live backends are wired where CPN already has logic (mailboxes, selective backups, MariaDB detect/CRUD, port change, systemctl/ps/php/docker/files/DNS).
- Unfinished ops use honest "Not configured yet" / scaffold UI (never fake success). Roadmap: `to-do/HOSTING-PANEL-FEATURE-ROADMAP.md`.

## Sidebar map
- **Hosting:** Dashboard, Websites, Email, Databases & FTP, Backups, Apps, Plugins
- **Account:** Users & Plans (stub; Packages #86 can coexist)
- **Administration:** Server, Security (stub), Settings (Change Port)

## Live vs scaffold
See roadmap table. Highlights:
- **Live:** Services Status, Top Processes, PHP read-only, Package Manager search, Docker detect/list, Root File Manager (allowlisted), DNS zone/NS JSON, Change Port, DB list/create/delete, MariaDB Manager, FTP detect, Email accounts/webmail/delivery/forwarding/catchall/DKIM stores, Backups create/restore list/schedule/destinations JSON
- **Scaffold:** GDrive/remote backups, FTP CRUD/reset, most anti-spam email tiles, Security, Users & Plans

## Test plan
- [x] `cargo test --lib` (91 passed) including path traversal, service allowlist, DNS/DB sanitization
- [ ] Redeploy AL9 `:2087` and smoke hub pages in browser after binary deploy
- [ ] Confirm non-admin cannot control services or browse root files
- [ ] Confirm `/email` and `/databases` hubs link to existing mailbox/DB flows
- [ ] Coordinate rebase with #86 (Packages) and #88 (MariaDB+PMA) if both land

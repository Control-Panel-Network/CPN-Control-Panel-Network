# CPN Apps

Host application management from the Panel **Apps** sidebar and the `cpn app` CLI.

## Apps supported

| Id | Label | Packages (dnf / apt) | Notes |
|---|---|---|---|
| `mariadb` | MariaDB | `mariadb-server` | Mutual exclusion with MySQL |
| `mysql` | MySQL | `mysql-server` | Mutual exclusion with MariaDB |
| `phpmyadmin` | phpMyAdmin | `phpMyAdmin` / `phpmyadmin` | Installs packages; wire a vhost separately |
| `email` | Email (Postfix + Dovecot) | `postfix` + `dovecot` (+ apt IMAP pkgs) | Same stack family as installer mail backend |
| `rabbitmq` | RabbitMQ | `rabbitmq-server` | Enables `rabbitmq-server` unit |

## MariaDB XOR MySQL

On one host, MariaDB and MySQL typically conflict (same ports and overlapping packages). CPN **refuses** installing MariaDB while MySQL is present, and refuses MySQL while MariaDB is present. The Apps UI also shows a warning when the other engine is detected.

## Panel UI

- Sidebar: **Apps** (session-gated, same responsive shell as other panel pages)
- Per app: status (not installed / installed / running), **Install**, **Reinstall**, **Uninstall** (browser confirm)
- Success and error messages appear as notices on `/apps`

## CLI

```bash
sudo cpn app list
sudo cpn app install --name mariadb
sudo cpn app reinstall --name rabbitmq --yes
sudo cpn app uninstall --name phpmyadmin --yes
```

Names: `mariadb`, `mysql`, `phpmyadmin`, `email`, `rabbitmq`.

Mutations require root (or `CPN_ALLOW_NONROOT=1` for lab data dirs).

## Implementation

- Detection and recipes: `src/apps.rs`
- Panel HTML: `src/panel_apps.rs`
- Routes: `/apps`, `/apps/install`, `/apps/reinstall`, `/apps/uninstall` in `src/panel_routes.rs`
- CLI: `src/cli_apps.rs` + `src/bin/cpn.rs`

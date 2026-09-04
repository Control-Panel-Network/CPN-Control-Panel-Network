# Apps

Host application management from the Panel **Apps** sidebar and the `cpn app` CLI.

## Apps supported

| Id | Label | Packages (dnf / apt) | Notes |
|---|---|---|---|
| `mariadb` | MariaDB | `mariadb-server` | Mutual exclusion with MySQL; host-wide |
| `mysql` | MySQL | `mysql-server` | Mutual exclusion with MariaDB; host-wide |
| `phpmyadmin` | phpMyAdmin | `phpMyAdmin` / `phpmyadmin` | Host packages plus optional site path under `/home/<domain>/apps/phpmyadmin` |
| `email` | Email (Postfix + Dovecot) | `postfix` + `dovecot` | Host MTA/IMAP; optional `/home/<domain>/apps/webmail` |
| `rabbitmq` | RabbitMQ | `rabbitmq-server` | Host-wide; optional domain association for ACL/display |

## Domain and subdomain targeting

Filesystem key is the **site FQDN**, not the panel username:

- Apex: `/home/<domain>/apps/...`
- Subdomain: `/home/<parent>/<sub.fqdn>/apps/...`

Panel Apps shows a domain/subdomain picker limited to sites the session user owns or is granted (see `site-acl.json`). MariaDB/MySQL/RabbitMQ stay system packages; selecting a site only records an association.

## MariaDB XOR MySQL

CPN refuses installing MariaDB while MySQL is present, and the reverse.

## Postfix default MTA

When outbound SMTP is skipped at account setup, Linux guests install and enable **Postfix** automatically and persist localhost SMTP (`127.0.0.1:25` or `:587`). See `to-do/EMAIL-POSTFIX-DEFAULT.md`.

## CLI

```bash
sudo cpn app list
sudo cpn app install --name mariadb
sudo cpn app install --name phpmyadmin --domain example.com
sudo cpn app install --name email --domain example.com --subdomain blog
sudo cpn app reinstall --name rabbitmq --yes
sudo cpn app uninstall --name phpmyadmin --domain example.com --yes
```

Mutations require root (or `CPN_ALLOW_NONROOT=1` for lab data dirs).

## Implementation

- Detection/recipes: `src/apps.rs`, `src/apps_pkg.rs`, `src/apps_site.rs`
- ACL: `src/site_acl.rs`
- Panel: `src/panel_apps.rs`, `src/panel_app_routes.rs`
- CLI: `src/cli_apps.rs`

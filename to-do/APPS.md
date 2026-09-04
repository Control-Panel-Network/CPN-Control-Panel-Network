# Apps

Host application management from the Panel **Apps** sidebar and the `cpn app` CLI.

## Apps supported

| Id | Label | Packages (dnf / apt) | Notes |
|---|---|---|---|
| `mariadb` | MariaDB | `mariadb-server` | Mutual exclusion with MySQL; host-wide; default stack |
| `mysql` | MySQL | `mysql-server` | Mutual exclusion with MariaDB; host-wide |
| `postgresql` | PostgreSQL | `postgresql-server` + `postgresql` / `postgresql` | Opt-in; can coexist with MariaDB/MySQL; detects `postgresql` unit and `:5432` |
| `phpmyadmin` | phpMyAdmin | `phpMyAdmin` / `phpmyadmin` | Host packages plus optional site path under `/home/<domain>/apps/phpmyadmin` |
| `email` | Email (Postfix + Dovecot) | `postfix` + `dovecot` | Host MTA/IMAP; optional `/home/<domain>/apps/webmail` |
| `rabbitmq` | RabbitMQ | `rabbitmq-server` | Host-wide; optional domain association for ACL/display |

## Domain and subdomain targeting

Filesystem key is the **site FQDN**, not the panel username:

- Apex: `/home/<domain>/apps/...`
- Subdomain: `/home/<parent>/<sub.fqdn>/apps/...`

Panel Apps shows a domain/subdomain picker limited to sites the session user owns or is granted (see `site-acl.json`). MariaDB/MySQL/PostgreSQL/RabbitMQ stay system packages; selecting a site only records an association.

## MariaDB XOR MySQL

CPN refuses installing MariaDB while MySQL is present, and the reverse. Detection ignores MariaDB unit aliases (`mysql` -> `mariadb.service`) so Apps does not report both as installed.

## PostgreSQL (opt-in)

PostgreSQL is **not** part of the default install stack. Fresh installs still default to MariaDB + phpMyAdmin. Operators install PostgreSQL from Apps or:

```bash
sudo cpn app install --name postgresql
sudo cpn app start --name postgresql
sudo cpn app stop --name postgresql
```

On RHEL-family hosts, install runs `postgresql-setup --initdb` when data is not initialized yet. MariaDB and PostgreSQL may run on the same host.

The Apps card links to the Databases hub for related tooling. A store "Postgres Manager" plugin is optional and not required for core package install.

## Installer defaults

Fresh web-server installs install **MariaDB** and **phpMyAdmin** by default. Operators can pick MySQL, skip the engine, or skip phpMyAdmin in the installer UI, `/api/install/server`, or:

```bash
sudo cpn-installer --ensure-database-defaults
sudo cpn-installer --ensure-database-defaults --database mysql
sudo cpn-installer --ensure-database-defaults --database none --skip-phpmyadmin
```

See `to-do/DATABASE-DEFAULTS.md`.

## Postfix default MTA

When outbound SMTP is skipped at account setup, Linux guests install and enable **Postfix** automatically and persist localhost SMTP (`127.0.0.1:25` or `:587`). See `to-do/EMAIL-POSTFIX-DEFAULT.md`.

## CLI

```bash
sudo cpn app list
sudo cpn app install --name mariadb
sudo cpn app install --name postgresql
sudo cpn app start --name postgresql
sudo cpn app stop --name postgresql
sudo cpn app install --name phpmyadmin --domain example.com
sudo cpn app install --name email --domain example.com --subdomain blog
sudo cpn app reinstall --name rabbitmq --yes
sudo cpn app uninstall --name phpmyadmin --domain example.com --yes
```

Mutations require root (or `CPN_ALLOW_NONROOT=1` for lab data dirs).

## Implementation

- Detection/recipes: `src/apps.rs`, `src/apps_postgresql.rs`, `src/apps_pkg.rs`, `src/apps_site.rs`
- ACL: `src/site_acl.rs`
- Panel: `src/panel_apps.rs`, `src/panel_app_routes.rs`
- CLI: `src/cli_apps.rs`

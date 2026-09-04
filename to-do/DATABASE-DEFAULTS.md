# MariaDB + phpMyAdmin defaults

## Rule

Fresh Linux web-server installs (installer UI and `cpn-installer --ensure-database-defaults`) install **MariaDB** and **phpMyAdmin** unless the operator chooses otherwise.

## Defaults

| Setting | Default | Override |
|---|---|---|
| Database engine | MariaDB | UI radios, API `database`, CLI `--database mysql\|none` |
| phpMyAdmin | On | UI checkbox, API `install_phpmyadmin: false`, CLI `--skip-phpmyadmin` |

MariaDB and MySQL remain mutually exclusive (XOR). Detection ignores MariaDB's `mysql`/`mysqld` systemd aliases so Apps does not fake a MySQL install.

## Reachability

phpMyAdmin packages come from EPEL on dnf hosts. CPN wires a loopback nginx + php-fpm listener at `http://127.0.0.1:8081/` (same pattern as webmail on `:8080`). Panel **Databases** and **Apps** show honest status.

## Related

- Apps CLI: `to-do/APPS.md`
- Postfix default MTA: `to-do/EMAIL-POSTFIX-DEFAULT.md`

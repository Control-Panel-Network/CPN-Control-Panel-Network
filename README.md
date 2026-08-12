# CPN — Control Panel Network

> [!WARNING]
> **Proyecto en desarrollo (no terminado).** Esta versión es experimental y no está lista para servidores de producción.

[![Estado: en desarrollo](https://img.shields.io/badge/estado-en%20desarrollo-f59e0b)](#estado-del-proyecto)
[![CI](https://github.com/KraoESPfan1n/CPN-Control-Panel-Network/actions/workflows/ci.yml/badge.svg)](https://github.com/KraoESPfan1n/CPN-Control-Panel-Network/actions/workflows/ci.yml)
[![Licencia: GPL v3](https://img.shields.io/badge/licencia-GPLv3-blue.svg)](LICENSE)

CPN es un instalador web para preparar componentes de un panel de servidores en AlmaLinux 9. Un único proceso escrito en Rust abre la interfaz HTTP, comunica el progreso real mediante WebSockets e incorpora la aplicación React dentro del ejecutable final.

## Estado del proyecto

La primera fase implementa el flujo del instalador y continúa en desarrollo. Actualmente incluye:

- Selección e instalación de OpenLiteSpeed, Caddy o Nginx.
- Selección de SnappyMail, Roundcube o Thunderbird. RainLoop se retiró por estar archivado y sin mantenimiento.
- Validación real del dominio y elección entre DNS local o Cloudflare mediante OAuth.
- Progreso real de descarga, instalación y comprobación enviado por WebSocket.
- Acceso local por defecto; `--allow-remote` abre temporalmente `8787` y limpia la regla al finalizar.
- Empaquetado RPM para AlmaLinux 9.
- Pruebas de servicios en contenedores limpios de AlmaLinux 9.8.

Las recetas, la seguridad y la compatibilidad aún deben revisarse antes de considerar CPN apto para producción.

## Estructura

- `installer-ui/`: interfaz React y Vite.
- `Panel/`: panel de control React y Next.js basado en las pantallas de Stitch.
- `src/`: servidor Actix Web, WebSocket, detección del entorno y recetas de instalación.
- `packaging/`: especificación RPM.
- `scripts/build-rpm.sh`: creación del binario y del RPM en AlmaLinux 9.
- `tests/docker-matrix.sh`: matriz funcional de servidores web y clientes de correo.

## Desarrollo y validación

```bash
# Rust
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings

# React
cd installer-ui
npm ci
npm run lint
npm run build

# Panel Next.js
cd ../Panel
npm ci
npm run lint
npm run typecheck
npm run build
```

La acción de integración continua ejecuta estas comprobaciones para cada cambio enviado y cada pull request.

## Empaquetado RPM

En AlmaLinux 9:

```bash
./scripts/build-rpm.sh
sudo dnf install ./target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
sudo cpn-installer                       # acceso local, recomendado con túnel SSH
sudo cpn-installer --allow-remote        # acceso remoto explícito
```

Al ejecutar `cpn-installer`, la consola indica inmediatamente la URL completa. El enlace de arranque solo puede usarse una vez; después se sustituye por una cookie HttpOnly y la URL queda limpia. Por defecto escucha únicamente en `127.0.0.1:8787`.

## Pruebas funcionales en Docker

La matriz requiere Docker con soporte para contenedores privilegiados y systemd:

```bash
./tests/docker-matrix.sh
```

Comprueba que Nginx, Caddy y OpenLiteSpeed respondan por HTTP, y que SnappyMail, Roundcube y Thunderbird superen verificaciones específicas.

SnappyMail y Roundcube son clientes web, no un servidor de correo completo. Esta faceta no declara SMTP/IMAP operativo hasta que CPN incorpore y pruebe un backend de correo separado. Thunderbird es un cliente de escritorio y no publica una URL web.

## Seguridad

No publiques el enlace de arranque temporal. CPN realiza cambios de sistema y debe ejecutarse únicamente en una máquina de pruebas dedicada durante esta etapa. Los artefactos externos soportados tienen versión y SHA-256 fijados y se descargan en temporales privados.

## Licencia

Copyright (C) 2026 CPN contributors.

Este proyecto se distribuye bajo la [GNU General Public License versión 3](LICENSE) (`GPL-3.0-only`).

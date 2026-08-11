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
- Selección e instalación de SnappyMail, RainLoop, Roundcube o Thunderbird.
- Progreso real de descarga, instalación y comprobación enviado por WebSocket.
- Detección de VPS y apertura del puerto `8787` en `firewalld` o `ufw` cuando están activos.
- Empaquetado RPM para AlmaLinux 9.
- Pruebas de servicios en contenedores limpios de AlmaLinux 9.8.

Las recetas, la seguridad y la compatibilidad aún deben revisarse antes de considerar CPN apto para producción.

## Estructura

- `installer-ui/`: interfaz React y Vite.
- `src/`: servidor Actix Web, WebSocket, detección del entorno y recetas de instalación.
- `packaging/`: especificación RPM.
- `scripts/build-rpm.sh`: creación del binario y del RPM en AlmaLinux 9.
- `tests/docker-matrix.sh`: matriz funcional de servidores web y clientes de correo.

## Desarrollo y validación

```bash
# Rust
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- -D warnings

# React, sin generar un build de producción
cd installer-ui
npm ci
npm run lint
```

La acción de integración continua ejecuta estas comprobaciones para cada cambio enviado y cada pull request.

## Empaquetado RPM

En AlmaLinux 9:

```bash
./scripts/build-rpm.sh
sudo dnf install ./target/rpmbuild/RPMS/x86_64/cpn-installer-*.rpm
sudo cpn-installer
```

Al ejecutar `cpn-installer`, la consola indica inmediatamente la URL completa del instalador web, incluida la IP accesible y un token temporal. El servicio escucha en `0.0.0.0:8787`.

## Pruebas funcionales en Docker

La matriz requiere Docker con soporte para contenedores privilegiados y systemd:

```bash
./tests/docker-matrix.sh
```

Comprueba que Nginx y Caddy respondan por HTTP, y que SnappyMail, RainLoop, Roundcube y Thunderbird queden instalados y superen sus verificaciones específicas.

## Seguridad

No publiques el token temporal que aparece en la URL del instalador. CPN realiza cambios de sistema y debe ejecutarse únicamente en una máquina de pruebas dedicada durante esta etapa.

## Licencia

Copyright (C) 2026 CPN contributors.

Este proyecto se distribuye bajo la [GNU General Public License versión 3](LICENSE) (`GPL-3.0-only`).

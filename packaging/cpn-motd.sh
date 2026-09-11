#!/bin/bash
# CPN / Control Panel Network interactive login banner (English).
# Installed to /etc/profile.d/cpn-motd.sh after a successful CPN install.
# Login URL(s) are resolved LIVE each login via `cpn panel url --motd`
# (reads /var/lib/cpn/listen_port, panel_public_url, panel_hostname).
# Safe for AlmaLinux / RHEL-family and Debian/Ubuntu login shells.
# Never prints passwords or tokens. Product branding: CPN / Control Panel Network only.

# Only interactive shells (skip scp/sftp/non-TTY).
case $- in
  *i*) ;;
  *) return 0 2>/dev/null || exit 0 ;;
esac
if [ ! -t 1 ]; then
  return 0 2>/dev/null || exit 0
fi

CPN_DATA_DIR="${CPN_DATA_DIR:-/var/lib/cpn}"

_cpn_color=0
if [ -n "${TERM:-}" ] && [ "${TERM}" != "dumb" ] && command -v tput >/dev/null 2>&1; then
  if [ "$(tput colors 2>/dev/null || echo 0)" -ge 8 ] 2>/dev/null; then
    _cpn_color=1
  fi
fi
if [ "${_cpn_color}" -eq 1 ]; then
  _C_CYAN="$(printf '\033[1;36m')"
  _C_BLUE="$(printf '\033[1;34m')"
  _C_RESET="$(printf '\033[0m')"
  _C_DIM="$(printf '\033[2m')"
else
  _C_CYAN=""
  _C_BLUE=""
  _C_RESET=""
  _C_DIM=""
fi

CPN_VERSION="unknown"
if command -v cpn >/dev/null 2>&1; then
  CPN_VERSION="$(cpn version 2>/dev/null | awk '{print $NF; exit}')"
  [ -z "${CPN_VERSION}" ] && CPN_VERSION="unknown"
elif command -v cpn-installer >/dev/null 2>&1; then
  CPN_VERSION="$(cpn-installer --version 2>/dev/null | awk '{print $NF; exit}')"
  [ -z "${CPN_VERSION}" ] && CPN_VERSION="unknown"
fi

_cpn_now="$(date '+%d/%m/%Y %H:%M:%S %Z' 2>/dev/null || date)"
_cpn_load="$(awk '{print $1", "$2", "$3}' /proc/loadavg 2>/dev/null || echo "n/a")"
_cpn_uptime="n/a"
if [ -r /proc/uptime ]; then
  _cpn_secs="$(awk '{print int($1)}' /proc/uptime 2>/dev/null || echo 0)"
  _cpn_d=$((_cpn_secs / 86400))
  _cpn_h=$(((_cpn_secs % 86400) / 3600))
  _cpn_m=$(((_cpn_secs % 3600) / 60))
  if [ "${_cpn_d}" -gt 0 ]; then
    _cpn_uptime="${_cpn_d}d ${_cpn_h}h ${_cpn_m}m"
  elif [ "${_cpn_h}" -gt 0 ]; then
    _cpn_uptime="${_cpn_h}h ${_cpn_m}m"
  else
    _cpn_uptime="${_cpn_m}m"
  fi
fi

_cpn_mem="n/a"
if [ -r /proc/meminfo ]; then
  _cpn_mt="$(awk '/^MemTotal:/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
  _cpn_ma="$(awk '/^MemAvailable:/ {print $2}' /proc/meminfo 2>/dev/null || echo 0)"
  if [ "${_cpn_mt}" -gt 0 ] 2>/dev/null; then
    _cpn_mu=$((_cpn_mt - _cpn_ma))
    _cpn_mt_mb=$((_cpn_mt / 1024))
    _cpn_mu_mb=$((_cpn_mu / 1024))
    _cpn_mp=$(((_cpn_mu * 100) / _cpn_mt))
    _cpn_mem="${_cpn_mu_mb} / ${_cpn_mt_mb} MB (${_cpn_mp}%)"
  fi
fi

_cpn_disk="n/a"
_cpn_df="$(df -P / 2>/dev/null | awk 'NR==2 {print $3" "$2" "$5}')"
if [ -n "${_cpn_df}" ]; then
  _cpn_du_k="$(echo "${_cpn_df}" | awk '{print $1}')"
  _cpn_dt_k="$(echo "${_cpn_df}" | awk '{print $2}')"
  _cpn_dp="$(echo "${_cpn_df}" | awk '{print $3}')"
  _cpn_du_g="$(awk -v k="${_cpn_du_k}" 'BEGIN { printf "%.1f", k/1048576 }')"
  _cpn_dt_g="$(awk -v k="${_cpn_dt_k}" 'BEGIN { printf "%.1f", k/1048576 }')"
  _cpn_disk="${_cpn_du_g} / ${_cpn_dt_g} GB (${_cpn_dp})"
fi

_cpn_cpu="n/a"
_cpn_nproc="$(nproc 2>/dev/null || echo 1)"
_cpn_load1="$(awk '{print $1}' /proc/loadavg 2>/dev/null || echo "")"
if [ -n "${_cpn_load1}" ] && [ "${_cpn_nproc}" -gt 0 ] 2>/dev/null; then
  _cpn_cpu="$(awk -v l="${_cpn_load1}" -v n="${_cpn_nproc}" 'BEGIN {
    v = (l * 100) / n;
    if (v < 0) v = 0;
    if (v > 999) v = 999;
    printf "%d%%", v;
  }')"
fi

_cpn_svc="unknown"
if command -v systemctl >/dev/null 2>&1; then
  _cpn_svc="$(systemctl is-active cpn-installer.service 2>/dev/null || echo unknown)"
fi

# Live login URLs: prefer operator CLI (same resolution as panel UI writes).
_cpn_print_live_urls() {
  if command -v cpn >/dev/null 2>&1; then
    if cpn panel url --motd 2>/dev/null; then
      return 0
    fi
  fi
  # Fallback: world-readable /etc/cpn mirror first, then root-only data dir.
  _cpn_facts_dir="/etc/cpn"
  if [ ! -r "${_cpn_facts_dir}/listen_port" ] && [ -r "${CPN_DATA_DIR}/listen_port" ]; then
    _cpn_facts_dir="${CPN_DATA_DIR}"
  fi
  _cpn_port="2087"
  if [ -r "${_cpn_facts_dir}/listen_port" ]; then
    _cpn_port_raw="$(tr -d '[:space:]' < "${_cpn_facts_dir}/listen_port" 2>/dev/null || true)"
    case "${_cpn_port_raw}" in
      ''|*[!0-9]*) ;;
      *) _cpn_port="${_cpn_port_raw}" ;;
    esac
  elif [ -r "${CPN_DATA_DIR}/listen_port" ]; then
    _cpn_port_raw="$(tr -d '[:space:]' < "${CPN_DATA_DIR}/listen_port" 2>/dev/null || true)"
    case "${_cpn_port_raw}" in
      ''|*[!0-9]*) ;;
      *) _cpn_port="${_cpn_port_raw}" ;;
    esac
  fi
  _cpn_hostname=""
  if [ -r "${_cpn_facts_dir}/panel_hostname" ]; then
    _cpn_hostname="$(tr -d '[:space:]' < "${_cpn_facts_dir}/panel_hostname" 2>/dev/null || true)"
  elif [ -r "${CPN_DATA_DIR}/panel_hostname" ]; then
    _cpn_hostname="$(tr -d '[:space:]' < "${CPN_DATA_DIR}/panel_hostname" 2>/dev/null || true)"
  fi
  _cpn_public=""
  if [ -r "${_cpn_facts_dir}/panel_public_url" ]; then
    _cpn_public="$(tr -d '\r\n' < "${_cpn_facts_dir}/panel_public_url" 2>/dev/null | sed 's/[[:space:]]*$//' || true)"
    _cpn_public="${_cpn_public%/}"
  elif [ -r "${CPN_DATA_DIR}/panel_public_url" ]; then
    _cpn_public="$(tr -d '\r\n' < "${CPN_DATA_DIR}/panel_public_url" 2>/dev/null | sed 's/[[:space:]]*$//' || true)"
    _cpn_public="${_cpn_public%/}"
  fi
  if [ -n "${_cpn_public}" ]; then
    echo "  Login URL     : ${_cpn_public}/login"
    echo "  Local login   : http://127.0.0.1:${_cpn_port}/login"
    echo "  Listen port   : ${_cpn_port}"
    echo "  Public URL    : ${_cpn_public}"
    echo "  Lab tip       : Windows host may use the public URL when NAT forwards the guest port"
  elif [ -n "${_cpn_hostname}" ]; then
    echo "  Login URL     : https://${_cpn_hostname}/login"
    echo "  Local login   : http://127.0.0.1:${_cpn_port}/login"
    echo "  Listen port   : ${_cpn_port}"
  else
    echo "  Login URL     : http://127.0.0.1:${_cpn_port}/login"
    echo "  Listen port   : ${_cpn_port}"
    echo "  Lab tip       : ssh -L ${_cpn_port}:127.0.0.1:${_cpn_port} user@host"
    echo "  VBox NAT tip  : host forward 2089->${_cpn_port} => http://127.0.0.1:2089/login"
  fi
}

echo ""
echo "${_C_CYAN}   ____ ____  _   ${_C_RESET}"
echo "${_C_CYAN}  / ___|  _ \\| \\ | |${_C_RESET}"
echo "${_C_BLUE} | |   | |_) |  \\| |${_C_RESET}"
echo "${_C_BLUE} | |___|  __/| |\\  |${_C_RESET}"
echo "${_C_CYAN}  \\____|_|   |_| \\_|${_C_RESET}"
echo "${_C_DIM}  Control Panel Network  ·  News Targeted${_C_RESET}"
echo "============================================================"
echo "  This server has installed CPN"
echo "  Panel version : ${CPN_VERSION}"
echo "  Panel service : ${_cpn_svc}"
_cpn_print_live_urls
echo "  Start panel   : sudo systemctl start cpn-installer.service"
echo "  Panel status  : cpn panel status   (or: systemctl status cpn-installer.service)"
echo "  Show URL anytime: cpn panel url"
echo "  CLI install   : sudo cpn-installer --cli"
echo "  Web start     : sudo cpn-installer --web"
echo "------------------------------------------------------------"
echo "  Time          : ${_cpn_now}"
echo "  Load average  : ${_cpn_load}"
echo "  CPU (load)    : ${_cpn_cpu}"
echo "  Memory        : ${_cpn_mem}"
echo "  Disk (/)      : ${_cpn_disk}"
echo "  Uptime        : ${_cpn_uptime}"

if command -v last >/dev/null 2>&1 && [ -n "${USER:-}" ]; then
  _cpn_last="$(last -1 -R "${USER}" 2>/dev/null | head -1 | tr -s ' ')"
  case "${_cpn_last}" in
    ''|wtmp*|reboot*|shutdown*) ;;
    *) echo "  Last login    : ${_cpn_last}" ;;
  esac
fi

_cpn_authlog=""
[ -r /var/log/secure ] && _cpn_authlog="/var/log/secure"
[ -r /var/log/auth.log ] && _cpn_authlog="/var/log/auth.log"
if [ -n "${_cpn_authlog}" ]; then
  _cpn_fails="$(tail -n 200 "${_cpn_authlog}" 2>/dev/null | grep -cE 'Failed password|authentication failure' || true)"
  if [ -n "${_cpn_fails}" ] && [ "${_cpn_fails}" -gt 0 ] 2>/dev/null; then
    echo "  Auth fails    : ${_cpn_fails} in recent auth log lines"
  fi
fi

echo "============================================================"
echo ""

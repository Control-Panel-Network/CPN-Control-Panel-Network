#!/bin/bash
# CPN / Control Panel Network interactive login banner (English).
# Installed to /etc/profile.d/cpn-motd.sh after a successful CPN install.
# Safe for AlmaLinux / RHEL-family and Debian/Ubuntu login shells.

# Only interactive shells (skip scp/sftp/non-TTY).
case $- in
  *i*) ;;
  *) return 0 2>/dev/null || exit 0 ;;
esac
if [ ! -t 1 ]; then
  return 0 2>/dev/null || exit 0
fi

CPN_DATA_DIR="${CPN_DATA_DIR:-/var/lib/cpn}"
CPN_PORT="2087"
if [ -r "${CPN_DATA_DIR}/listen_port" ]; then
  _cpn_port_raw="$(tr -d '[:space:]' < "${CPN_DATA_DIR}/listen_port" 2>/dev/null || true)"
  case "${_cpn_port_raw}" in
    ''|*[!0-9]*) ;;
    *) CPN_PORT="${_cpn_port_raw}" ;;
  esac
fi

CPN_HOSTNAME=""
if [ -r "${CPN_DATA_DIR}/panel_hostname" ]; then
  CPN_HOSTNAME="$(tr -d '[:space:]' < "${CPN_DATA_DIR}/panel_hostname" 2>/dev/null || true)"
fi

CPN_VERSION="unknown"
if command -v cpn-installer >/dev/null 2>&1; then
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

# Load relative to CPU count (fast; no sampling delay).
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

echo ""
echo "============================================================"
echo "  CPN / Control Panel Network"
echo "  News Targeted"
echo "============================================================"
echo "  Panel version : ${CPN_VERSION}"
if [ -n "${CPN_HOSTNAME}" ]; then
  echo "  Panel login   : https://${CPN_HOSTNAME}/login"
  echo "  Local login   : http://127.0.0.1:${CPN_PORT}/login"
else
  echo "  Panel login   : http://127.0.0.1:${CPN_PORT}/login"
  echo "  Lab tip       : ssh -L ${CPN_PORT}:127.0.0.1:${CPN_PORT} user@host"
  echo "  VBox NAT tip  : host forward 2089->${CPN_PORT} => http://127.0.0.1:2089/login"
fi
echo "  Start panel   : systemctl start cpn-installer.service"
echo "  Panel status  : systemctl status cpn-installer.service"
echo "  CLI install   : sudo cpn-installer --cli"
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

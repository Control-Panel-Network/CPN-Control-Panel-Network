//! Client script for Version Management (live retry countdown + release picker).

use crate::panel_hub_pages_version_eta_script::version_page_eta_script;
use crate::panel_hub_pages_version_log_script::version_page_log_script;
use crate::panel_hub_pages_version_poll_script::version_page_poll_script;
use crate::panel_hub_pages_version_source_script::version_fetch_helpers_script;
use crate::panel_hub_pages_version_ui_script::version_page_ui_script;

/// Inline `<script>` for `/settings/version`. `can_manage` gates upgrade/repair UI.
pub fn version_page_script(can_manage: bool) -> String {
    let can_manage_js = if can_manage { "true" } else { "false" };
    format!(
        r##"{helpers}<script>
(function () {{
  var canManage = {can_manage_js};
  var statusEl = document.getElementById("cpn-version-status");
  var btn = document.getElementById("cpn-version-refresh");
  var runningEl = document.getElementById("cpn-version-running");
  var latestEl = document.getElementById("cpn-version-latest");
  var sourceTipEl = document.getElementById("cpn-version-source-tip");
  var upstreamTipEl = document.getElementById("cpn-version-upstream-tip");
  var installedEl = document.getElementById("cpn-version-installed");
  var searchEl = document.getElementById("cpn-version-search");
  var resultsEl = document.getElementById("cpn-version-results");
  var selectedLabel = document.getElementById("cpn-version-selected-label");
  var selectedTag = "";
  var releaseCache = [];
  var confirmBox = document.getElementById("cpn-version-confirm");
  var confirmText = document.getElementById("cpn-version-confirm-text");
  var confirmGo = document.getElementById("cpn-version-confirm-go");
  var confirmCancel = document.getElementById("cpn-version-confirm-cancel");
  var progressWrap = document.getElementById("cpn-version-progress-wrap");
  var progressBar = document.getElementById("cpn-version-progress-bar");
  var progressLabel = document.getElementById("cpn-version-progress-label");
  var opError = document.getElementById("cpn-version-op-error");
  var infoCache = null;
  var pending = null;
  var pollTimer = null;
  var retryTimer = null;
  var retryLeft = 0;
  var busy = false;

  function esc(s) {{
    return String(s == null ? "" : s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }}
  function friendlyFetchError(err, context) {{
    return window.cpnFriendlyFetchError(err, context);
  }}
  function norm(v) {{
    return String(v || "").replace(/^v/i, "").trim();
  }}
  function cmp(a, b) {{
    var as = norm(a).split(".");
    var bs = norm(b).split(".");
    var n = Math.max(as.length, bs.length);
    for (var i = 0; i < n; i++) {{
      var ai = parseInt(as[i] || "0", 10) || 0;
      var bi = parseInt(bs[i] || "0", 10) || 0;
      if (ai < bi) return -1;
      if (ai > bi) return 1;
    }}
    return 0;
  }}
  function formatPublishedAt(iso) {{
    var raw = String(iso || "").trim();
    if (!raw) return "-";
    var datePart = raw.split("T")[0] || raw;
    var parts = datePart.split("-");
    if (parts.length !== 3 || parts[0].length !== 4) return raw;
    var out = parts[2] + "/" + parts[1] + "/" + parts[0];
    var timePart = raw.split("T")[1] || "";
    if (timePart.length >= 5) {{
      var hm = timePart.substring(0, 5);
      if (hm.charAt(2) === ":") out += " " + hm;
    }}
    return out;
  }}
  function formatInstalledAt(unix) {{
    var ts = Number(unix);
    if (!ts || !isFinite(ts) || ts <= 0) return "";
    var d = new Date(ts * 1000);
    if (isNaN(d.getTime())) return "";
    function pad(n) {{ return String(n).padStart(2, "0"); }}
    return pad(d.getDate()) + "/" + pad(d.getMonth() + 1) + "/" + d.getFullYear()
      + " " + pad(d.getHours()) + ":" + pad(d.getMinutes());
  }}
  function paintInstalledVersionColor(state) {{
    if (!installedEl) return;
    if (state === "stale_behind" || state === "stale") {{
      installedEl.style.color = "#f87171";
      installedEl.setAttribute("data-update-state", "stale");
    }} else if (state === "update_available" || state === "behind") {{
      installedEl.style.color = "#fb923c";
      installedEl.setAttribute("data-update-state", "behind");
    }} else if (state === "current") {{
      installedEl.style.color = "#4ade80";
      installedEl.setAttribute("data-update-state", "current");
    }} else {{
      installedEl.style.color = "";
      installedEl.removeAttribute("data-update-state");
    }}
  }}
  function isPrereleaseLabel(ver) {{
    var s = String(ver || "").toLowerCase();
    return /(alpha|beta|rc|dev|pre|preview|snapshot|nightly)/.test(s);
  }}
  function releaseIsPrerelease(r) {{
    if (!r) return false;
    if (r.prerelease === true) return true;
    return isPrereleaseLabel(r.version) || isPrereleaseLabel(r.tag_name);
  }}
  function resolveInstalledColor(info) {{
    if (info && info.installed_package_color) return String(info.installed_package_color);
    if (!info) return null;
    var installed = info.installed_version || "";
    var releases = info.releases || [];
    var onPre = isPrereleaseLabel(installed);
    var row = releaseRowForVersion(installed);
    if (row && releaseIsPrerelease(row)) onPre = true;
    var newerStable = false;
    var newerChannel = false;
    for (var i = 0; i < releases.length; i++) {{
      var r = releases[i];
      var ver = r.version || r.tag_name || "";
      if (!ver || cmp(installed, ver) >= 0) continue;
      var isPre = releaseIsPrerelease(r);
      if (!isPre) newerStable = true;
      if (onPre || !isPre) newerChannel = true;
    }}
    if (!!info.stable_update_available) newerChannel = true;
    var ts = Number(info.installed_at_unix || 0);
    var stale = false;
    if (ts > 0) {{
      var age = (Date.now() / 1000) - ts;
      stale = age > (183 * 24 * 60 * 60);
    }}
    if (stale && newerStable) return "stale_behind";
    if (newerChannel) return "update_available";
    return "current";
  }}
  function setDetailRow(rowId, valueId, text) {{
    var row = document.getElementById(rowId);
    var value = document.getElementById(valueId);
    var show = !!(text && String(text).trim());
    if (row) {{
      if (show) row.removeAttribute("hidden");
      else row.setAttribute("hidden", "hidden");
    }}
    if (value) value.textContent = show ? String(text) : "-";
  }}
  var CACHE_KEY = "cpnVersionCache";
  function saveVersionCache(info) {{
    try {{
      if (!info || typeof sessionStorage === "undefined") return;
      sessionStorage.setItem(CACHE_KEY, JSON.stringify({{ saved_at: Date.now(), info: info }}));
    }} catch (e) {{}}
  }}
  function loadVersionCache() {{
    try {{
      if (typeof sessionStorage === "undefined") return null;
      var raw = sessionStorage.getItem(CACHE_KEY);
      if (!raw) return null;
      var parsed = JSON.parse(raw);
      if (!parsed || !parsed.info) return null;
      return parsed.info;
    }} catch (e) {{
      return null;
    }}
  }}
  function setRowVisible(rowId, show) {{
    var row = document.getElementById(rowId);
    if (!row) return;
    if (show) row.removeAttribute("hidden");
    else row.setAttribute("hidden", "hidden");
  }}
  function paintDetailRows(info, liveRetry, hasTip) {{
    var showRepo = !!(info && info.repo && info.using_fork);
    setDetailRow("cpn-version-row-repo", "cpn-version-repo", showRepo ? info.repo : "");
    setDetailRow("cpn-version-row-source", "cpn-version-pkg-source", info && info.source);
    var notes = [];
    if (info && info.using_fork) notes.push("Using fork source for upgrades");
    if (info && info.token_configured) notes.push("GitHub token configured");
    if (info && info.from_cache) {{
      notes.push("Release list cached"
        + (info.cache_age_secs != null ? (" (" + info.cache_age_secs + "s old)") : ""));
    }}
    if (info && info.cache_note && !liveRetry) notes.push(info.cache_note);
    if (info && info.check_error && hasTip) notes.push("API note: " + info.check_error);
    if (info && info.tip_check_error) notes.push("Commit check: " + info.tip_check_error);
    setDetailRow("cpn-version-row-note", "cpn-version-note", notes.join(" · "));
  }}
  function releaseRowForVersion(ver) {{
    var want = norm(ver);
    if (!want || !releaseCache.length) return null;
    for (var i = 0; i < releaseCache.length; i++) {{
      var r = releaseCache[i];
      var v = norm(r.version || r.tag_name);
      var t = norm(r.tag_name || r.version);
      if (v === want || t === want) return r;
    }}
    return null;
  }}
  function shortSha(sha, source) {{
    if (!sha) return "";
    return "Commit " + String(sha).substring(0, 7)
      + (source ? (" (" + source + ")") : "");
  }}
  function paintIdentityRows(info) {{
    var runVer = (info && info.running_version) || (runningEl && runningEl.textContent) || "";
    var instVer = (info && info.installed_version) || (installedEl && installedEl.textContent) || "";
    var match = !!runVer && !!instVer && norm(runVer) === norm(instVer);
    var currentEl = document.getElementById("cpn-version-current");
    var divergeEl = document.getElementById("cpn-version-diverge");
    setRowVisible("cpn-version-row-current", match);
    setRowVisible("cpn-version-row-running", !match);
    setRowVisible("cpn-version-row-installed", !match);
    if (match) {{
      if (currentEl) currentEl.textContent = runVer;
      setRowVisible("cpn-version-row-diverge", false);
      if (divergeEl) divergeEl.textContent = "";
    }} else {{
      if (runningEl && runVer) runningEl.textContent = runVer;
      if (installedEl && instVer) installedEl.textContent = instVer;
      var note = "";
      if (runVer && instVer && cmp(runVer, instVer) > 0) {{
        note = "Running binary is ahead of packaged RPM/DEB. Upgrade the package to match the running binary.";
      }} else if (runVer && instVer) {{
        note = "Packaged install differs from the running binary.";
      }}
      if (divergeEl) divergeEl.textContent = note;
      setRowVisible("cpn-version-row-diverge", !!note);
    }}
  }}
  function paintReleaseDates(info) {{
    var runningDateEl = document.getElementById("cpn-version-running-date");
    var installedDateEl = document.getElementById("cpn-version-installed-date");
    var installedAtEl = document.getElementById("cpn-version-installed-at");
    var currentDateEl = document.getElementById("cpn-version-current-date");
    var currentAtEl = document.getElementById("cpn-version-current-installed-at");
    var currentShaEl = document.getElementById("cpn-version-current-sha");
    var runningShaEl = document.getElementById("cpn-version-running-sha");
    var runningRow = releaseRowForVersion(info && info.running_version);
    var installedRow = releaseRowForVersion(info && info.installed_version);
    var runDate = runningRow && runningRow.published_at
      ? ("Released " + formatPublishedAt(runningRow.published_at)) : "";
    var instDate = installedRow && installedRow.published_at
      ? ("Released " + formatPublishedAt(installedRow.published_at)) : "";
    var when = formatInstalledAt(info && info.installed_at_unix);
    var installedAtText = when ? ("Installed " + when) : "";
    var shaText = shortSha(info && info.running_sha, info && info.running_sha_source);
    if (runningDateEl) runningDateEl.textContent = runDate;
    if (installedDateEl) installedDateEl.textContent = instDate;
    if (installedAtEl) installedAtEl.textContent = installedAtText;
    if (runningShaEl) runningShaEl.textContent = shaText;
    if (currentDateEl) currentDateEl.textContent = runDate || instDate;
    if (currentAtEl) currentAtEl.textContent = installedAtText;
    if (currentShaEl) currentShaEl.textContent = shaText;
  }}
  function supportedReleaseTags() {{
    if (!releaseCache.length) return {{}};
    var sorted = releaseCache.slice().sort(function (a, b) {{
      var av = a.version || norm(a.tag_name);
      var bv = b.version || norm(b.tag_name);
      return -cmp(av, bv);
    }});
    var set = {{}};
    for (var i = 0; i < Math.min(2, sorted.length); i++) {{
      var tag = sorted[i].tag_name || sorted[i].version;
      if (tag) set[norm(tag)] = true;
    }}
    return set;
  }}
  function isOutsideSupport(tag) {{
    var key = norm(tag);
    if (!key) return false;
    var supported = supportedReleaseTags();
    if (!Object.keys(supported).length) return false;
    return !supported[key];
  }}
  function retryMessage(secs) {{
    return "Checked recently; showing cached results. Try again in " + secs + " seconds.";
  }}
  function syncRefreshButton() {{
    if (!btn) return;
    btn.disabled = busy || retryLeft > 0;
  }}
  function stopRetryCountdown() {{
    if (retryTimer) {{ clearInterval(retryTimer); retryTimer = null; }}
    retryLeft = 0;
    if (statusEl) statusEl.removeAttribute("data-retry-after");
    syncRefreshButton();
  }}
  function paintRetryCountdown() {{
    if (!statusEl) return;
    if (retryLeft > 0) {{
      statusEl.setAttribute("data-retry-after", String(retryLeft));
      statusEl.textContent = retryMessage(retryLeft);
    }} else {{
      statusEl.removeAttribute("data-retry-after");
      statusEl.textContent = "Checked recently; showing cached results. You can check for updates again.";
    }}
    syncRefreshButton();
  }}
  function startRetryCountdown(secs) {{
    stopRetryCountdown();
    retryLeft = Math.max(0, Math.floor(Number(secs) || 0));
    paintRetryCountdown();
    if (retryLeft <= 0) return;
    retryTimer = setInterval(function () {{
      retryLeft -= 1;
      if (retryLeft <= 0) {{
        stopRetryCountdown();
        paintRetryCountdown();
        return;
      }}
      paintRetryCountdown();
    }}, 1000);
  }}
  function setSelected(tag) {{
    selectedTag = tag || "";
    if (selectedLabel) selectedLabel.textContent = selectedTag || "-";
    if (searchEl && selectedTag && document.activeElement !== searchEl) {{
      searchEl.value = selectedTag;
    }}
  }}
  function setActionsEnabled(on) {{
    ["cpn-version-upgrade-latest", "cpn-version-upgrade-stable", "cpn-version-apply", "cpn-version-repair"].forEach(function (id) {{
      var el = document.getElementById(id);
      if (el) el.disabled = !on;
    }});
    if (!on) {{
      if (btn) btn.disabled = true;
    }} else {{
      syncRefreshButton();
    }}
    if (searchEl) searchEl.disabled = !on;
  }}
  function clearConfirm() {{
    pending = null;
    if (confirmBox) confirmBox.style.display = "none";
  }}
  function armConfirm(action, version, label) {{
    if (!canManage || busy) return;
    pending = {{ action: action, version: version || null }};
    var extra = "";
    if (version && isOutsideSupport(version)) {{
      extra = " Warning: this tag is outside CPN support (only the latest two releases are supported).";
    }}
    if (confirmText) confirmText.textContent = "About to " + label + "." + extra + " Click Confirm to start, or Cancel.";
    if (confirmGo) confirmGo.textContent = "Confirm " + label;
    if (confirmBox) confirmBox.style.display = "block";
    if (opError) opError.textContent = "";
  }}
{ui}
  window.cpnVersionRecheck = check;
{eta}
{log}
{poll}
  if (btn) btn.addEventListener("click", function () {{ check(true); }});
  if (canManage) {{
    var upLatest = document.getElementById("cpn-version-upgrade-latest");
    var applyBtn = document.getElementById("cpn-version-apply");
    var repairBtn = document.getElementById("cpn-version-repair");
    if (searchEl) {{
      searchEl.addEventListener("input", function () {{
        showResults(filterReleases(searchEl.value));
      }});
      searchEl.addEventListener("focus", function () {{
        showResults(filterReleases(searchEl.value));
      }});
      searchEl.addEventListener("keydown", function (ev) {{
        if (ev.key === "Escape") {{
          hideResults();
          return;
        }}
        if (ev.key === "Enter") {{
          ev.preventDefault();
          var matches = filterReleases(searchEl.value);
          if (matches.length) {{
            setSelected(matches[0].tag);
            hideResults();
          }}
        }}
      }});
      document.addEventListener("click", function (ev) {{
        if (!resultsEl || !searchEl) return;
        if (ev.target === searchEl || resultsEl.contains(ev.target)) return;
        hideResults();
      }});
    }}
    if (upLatest) upLatest.addEventListener("click", function () {{
      var latest = infoCache && (infoCache.latest_tag || infoCache.latest_version);
      armConfirm("upgrade", latest || null, "upgrade to latest release" + (latest ? (" (" + latest + ")") : ""));
    }});
    var upStable = document.getElementById("cpn-version-upgrade-stable");
    if (upStable) upStable.addEventListener("click", function () {{
      var tipToken = null;
      if (infoCache && infoCache.stable_tip_short) {{
        tipToken = "stable@" + infoCache.stable_tip_short;
      }} else {{
        tipToken = "stable";
      }}
      var label = (infoCache && infoCache.stable_tip_label) ? infoCache.stable_tip_label : "stable commits";
      armConfirm("upgrade", tipToken, "upgrade to " + label + " (latest commits)");
    }});
    if (applyBtn) applyBtn.addEventListener("click", function () {{
      var tag = selectedTag || (searchEl && searchEl.value);
      if (!tag) {{ if (opError) opError.textContent = "Select a release first."; return; }}
      var installed = infoCache && infoCache.installed_version ? infoCache.installed_version : "";
      var action = (installed && cmp(tag, installed) < 0) ? "downgrade" : "upgrade";
      armConfirm(action, tag, action + " to " + tag);
    }});
    if (repairBtn) repairBtn.addEventListener("click", function () {{
      var tag = selectedTag || (searchEl && searchEl.value);
      if (!tag) {{ if (opError) opError.textContent = "Select a release first."; return; }}
      armConfirm("repair", tag, "repair with " + tag);
    }});
    if (confirmGo) confirmGo.addEventListener("click", function () {{
      if (!pending) return;
      startJob(pending.action, pending.version);
    }});
    if (confirmCancel) confirmCancel.addEventListener("click", clearConfirm);
  }}
  var cached = loadVersionCache();
  if (cached) {{
    render(cached);
    if (statusEl && (!statusEl.textContent || statusEl.textContent.indexOf("Checking") === 0)) {{
      statusEl.textContent = "Updating status...";
    }}
  }}
  check(false);
  if (canManage && typeof resumeIfBusy === "function") {{ resumeIfBusy(); }}
}})();
</script>"##,
        helpers = version_fetch_helpers_script(),
        can_manage_js = can_manage_js,
        ui = version_page_ui_script(),
        eta = version_page_eta_script(),
        log = version_page_log_script(),
        poll = version_page_poll_script()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_has_live_retry_countdown() {
        let js = version_page_script(true);
        assert!(js.contains("data-retry-after"));
        assert!(js.contains("startRetryCountdown"));
        assert!(js.contains("retry_after_secs"));
        assert!(js.contains("cpnFriendlyFetchError"));
        assert!(js.contains("cpn-version-source-tip"));
        assert!(js.contains("awaitingReconnect"));
        assert!(js.contains("Waiting for panel after restart"));
        assert!(js.contains("window.location.origin"));
        assert!(js.contains("probeLoginThen"));
        assert!(js.contains("location.replace"));
        assert!(js.contains("Waiting for panel restart"));
        assert!(js.contains("cpn-version-stable-tip"));
        assert!(js.contains("stable_update_available"));
        assert!(js.contains("cpn-version-upgrade-stable"));
        assert!(js.contains("stable@"));
        assert!(js.contains("stale_behind"));
        assert!(js.contains("resolveInstalledColor"));
        assert!(js.contains("isPrereleaseLabel"));
        assert!(js.contains("cpnVersionCache"));
        assert!(js.contains("paintIdentityRows"));
        assert!(js.contains("resumeIfBusy"));
        assert!(js.contains("Running binary is ahead of packaged"));
        assert!(js.contains("Refreshing (keeping current values)"));
        assert!(js.contains("etaPaint"));
        assert!(js.contains("Estimating time left..."));
        assert!(js.contains("function paintInstallerLog(text, forceOpen, st)"));
        assert!(js.contains("copyInstallerLog"));
        assert!(js.contains("LOG_STAMP_RE"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

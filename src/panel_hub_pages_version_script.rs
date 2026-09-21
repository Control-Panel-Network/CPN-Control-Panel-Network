//! Client script for Version Management (live retry countdown + release picker).

use crate::panel_hub_pages_version_source_script::version_fetch_helpers_script;

/// Inline `<script>` for `/settings/version`. `can_manage` gates upgrade/repair UI.
pub fn version_page_script(can_manage: bool) -> String {
    let can_manage_js = if can_manage { "true" } else { "false" };
    format!(
        r#"{helpers}<script>
(function () {{
  var canManage = {can_manage_js};
  var statusEl = document.getElementById("cpn-version-status");
  var detailsEl = document.getElementById("cpn-version-details");
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
    ["cpn-version-upgrade-latest", "cpn-version-apply", "cpn-version-repair"].forEach(function (id) {{
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
    if (confirmText) confirmText.textContent = "About to " + label + ". Click Confirm to start, or Cancel.";
    if (confirmGo) confirmGo.textContent = "Confirm " + label;
    if (confirmBox) confirmBox.style.display = "block";
    if (opError) opError.textContent = "";
  }}
  function hideResults() {{
    if (!resultsEl) return;
    resultsEl.style.display = "none";
    resultsEl.innerHTML = "";
    if (searchEl) searchEl.setAttribute("aria-expanded", "false");
  }}
  function showResults(items) {{
    if (!resultsEl) return;
    resultsEl.innerHTML = "";
    if (!items.length) {{
      var empty = document.createElement("li");
      empty.className = "muted";
      empty.style.padding = "8px 10px";
      empty.textContent = "No matching releases";
      resultsEl.appendChild(empty);
      resultsEl.style.display = "block";
      if (searchEl) searchEl.setAttribute("aria-expanded", "true");
      return;
    }}
    items.forEach(function (item) {{
      var li = document.createElement("li");
      li.setAttribute("role", "option");
      li.tabIndex = -1;
      li.style.padding = "8px 10px";
      li.style.cursor = "pointer";
      li.style.borderBottom = "1px solid rgba(148,163,184,0.15)";
      li.dataset.tag = item.tag;
      li.textContent = item.label;
      li.addEventListener("mousedown", function (ev) {{
        ev.preventDefault();
        setSelected(item.tag);
        hideResults();
      }});
      resultsEl.appendChild(li);
    }});
    resultsEl.style.display = "block";
    if (searchEl) searchEl.setAttribute("aria-expanded", "true");
  }}
  function filterReleases(query) {{
    var q = String(query || "").trim().toLowerCase();
    var installed = infoCache ? norm(infoCache.installed_version) : "";
    var latest = infoCache ? norm(infoCache.latest_version || (infoCache.latest_tag || "")) : "";
    var out = [];
    releaseCache.forEach(function (r) {{
      var ver = r.version || norm(r.tag_name);
      var tag = r.tag_name || ver;
      var hay = (tag + " " + ver).toLowerCase();
      if (q && hay.indexOf(q) === -1) return;
      var marks = [];
      if (norm(ver) === installed || norm(tag) === installed) marks.push("installed");
      if (latest && (norm(ver) === latest || norm(tag) === latest)) marks.push("latest");
      out.push({{
        tag: tag,
        label: tag + (marks.length ? " (" + marks.join(", ") + ")" : "")
      }});
    }});
    return out;
  }}
  function fillPicker(info) {{
    releaseCache = info.releases || [];
    var installed = norm(info.installed_version);
    var latest = norm(info.latest_version || (info.latest_tag || ""));
    if (!releaseCache.length) {{
      setSelected("");
      if (searchEl) searchEl.placeholder = "No releases loaded";
      return;
    }}
    var prefer = "";
    if (latest) {{
      for (var i = 0; i < releaseCache.length; i++) {{
        var ver = releaseCache[i].version || norm(releaseCache[i].tag_name);
        var tag = releaseCache[i].tag_name || ver;
        if (norm(ver) === latest || norm(tag) === latest) {{
          prefer = tag;
          break;
        }}
      }}
    }}
    if (!prefer && installed) {{
      for (var j = 0; j < releaseCache.length; j++) {{
        var ver2 = releaseCache[j].version || norm(releaseCache[j].tag_name);
        var tag2 = releaseCache[j].tag_name || ver2;
        if (norm(ver2) === installed || norm(tag2) === installed) {{
          prefer = tag2;
          break;
        }}
      }}
    }}
    if (!prefer && releaseCache[0]) {{
      prefer = releaseCache[0].tag_name || releaseCache[0].version || "";
    }}
    setSelected(prefer);
    hideResults();
  }}
  function render(info) {{
    infoCache = info;
    if (!info) {{
      stopRetryCountdown();
      statusEl.textContent = "Could not load version information.";
      return;
    }}
    if (runningEl && info.running_version) runningEl.textContent = info.running_version;
    if (installedEl && info.installed_version) installedEl.textContent = info.installed_version;
    if (latestEl) latestEl.textContent = info.latest_version || info.latest_tag || "-";
    var sourceTip = info.latest_version || info.latest_tag || "-";
    if (sourceTipEl) {{
      if (info.using_fork) {{
        sourceTipEl.textContent = "fork tip " + sourceTip + (info.repo ? (" (" + info.repo + ")") : "");
      }} else {{
        sourceTipEl.textContent = "official " + sourceTip + (info.repo ? (" (" + info.repo + ")") : "");
      }}
    }}
    if (upstreamTipEl) {{
      if (info.using_fork) {{
        var up = info.upstream_latest_version || info.upstream_latest_tag || "-";
        upstreamTipEl.textContent = up + (info.upstream_repo ? (" (" + info.upstream_repo + ")") : "");
      }} else {{
        upstreamTipEl.textContent = sourceTip + (info.upstream_repo ? (" (" + info.upstream_repo + ")") : "");
      }}
    }}
    var hasTip = !!(info.latest_version || info.latest_tag || (info.releases && info.releases.length));
    var wait = Math.max(0, Math.floor(Number(info.retry_after_secs) || 0));
    var liveRetry = wait > 0;
    if (liveRetry) {{
      startRetryCountdown(wait);
    }} else {{
      stopRetryCountdown();
      if (info.check_error && !hasTip) {{
        statusEl.textContent = "Update check failed: " + info.check_error;
      }} else if (info.rate_limited && info.cache_note) {{
        statusEl.textContent = info.cache_note;
      }} else if (info.update_available) {{
        statusEl.textContent = "Update available: " + (info.latest_version || info.latest_tag || "newer release");
      }} else if (info.cache_note) {{
        statusEl.textContent = info.cache_note;
      }} else if (info.check_error && hasTip) {{
        statusEl.textContent = "Showing available release info. Note: " + info.check_error;
      }} else {{
        statusEl.textContent = "You are on the latest known release" +
          (info.latest_version ? (" (" + info.latest_version + ")") : "") + ".";
      }}
    }}
    var lines = [];
    if (info.repo) lines.push("Configured repo: " + info.repo);
    if (info.source) lines.push("Source: " + info.source);
    if (info.using_fork) lines.push("Using fork source for upgrades");
    if (info.token_configured) lines.push("GitHub token: configured");
    if (info.using_fork && (info.upstream_latest_version || info.upstream_latest_tag)) {{
      lines.push("Upstream official: " + (info.upstream_latest_version || info.upstream_latest_tag));
    }}
    if (info.latest_tag) lines.push("Latest tag: " + info.latest_tag);
    if (info.from_cache) lines.push("Release list: cached" + (info.cache_age_secs != null ? (" (" + info.cache_age_secs + "s old)") : ""));
    if (info.cache_note && !liveRetry) lines.push(info.cache_note);
    if (info.check_error && hasTip) lines.push("API note: " + info.check_error);
    detailsEl.innerHTML = lines.map(function (l) {{
      return "<p>" + esc(l) + "</p>";
    }}).join("");
    fillPicker(info);
  }}
  function check(forceRefresh) {{
    if (forceRefresh && retryLeft > 0) return;
    statusEl.textContent = forceRefresh ? "Refreshing release list..." : "Checking for updates...";
    var url = "/api/version-check" + (forceRefresh ? "?refresh=1" : "");
    fetch(url, {{
      credentials: "same-origin",
      headers: {{ "Accept": "application/json" }}
    }}).then(function (res) {{
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }}).then(render).catch(function (err) {{
      stopRetryCountdown();
      statusEl.textContent = friendlyFetchError(err, "Update check");
      if (latestEl && !latestEl.textContent) latestEl.textContent = "-";
    }});
  }}
  window.cpnVersionRecheck = check;
  var pollFailCount = 0;
  var pollBackoffMs = 500;
  var POLL_FAIL_SOFT_MAX = 8;
  var POLL_FAIL_HARD_MAX = 40;
  function schedulePoll(delayMs) {{
    if (pollTimer) {{ clearTimeout(pollTimer); pollTimer = null; }}
    pollTimer = setTimeout(pollStatus, Math.max(250, delayMs || 500));
  }}
  function finishPollOk() {{
    busy = false;
    setActionsEnabled(true);
    if (pollTimer) {{ clearTimeout(pollTimer); pollTimer = null; }}
    pollFailCount = 0;
    pollBackoffMs = 500;
  }}
  function pollStatus() {{
    fetch("/api/maintenance/status", {{
      credentials: "same-origin",
      headers: {{ "Accept": "application/json" }},
      cache: "no-store"
    }}).then(function (res) {{
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }}).then(function (st) {{
      pollFailCount = 0;
      pollBackoffMs = 500;
      if (opError && opError.textContent.indexOf("Status poll failed") === 0) {{
        opError.textContent = "";
      }}
      var pct = Math.max(0, Math.min(100, Math.round(Number(st.progress) || 0)));
      if (progressBar) progressBar.style.width = pct + "%";
      if (progressLabel) {{
        var body = (st.phase || "") + (st.message ? (": " + st.message) : "");
        progressLabel.textContent = pct + "%" + (body ? (" " + body) : "");
      }}
      if (st.error) {{
        finishPollOk();
        if (opError) opError.textContent = st.error;
        if (progressLabel) progressLabel.textContent = pct + "% Failed: " + st.error;
        return;
      }}
      if (!st.busy && (st.phase === "completed" || st.phase === "ready" || st.phase === "failed")) {{
        finishPollOk();
        if (st.phase === "failed") {{
          if (opError) opError.textContent = st.error || "Maintenance failed";
        }} else {{
          if (progressBar) progressBar.style.width = "100%";
          if (progressLabel) progressLabel.textContent = "100% Completed.";
          check(false);
        }}
        return;
      }}
      schedulePoll(500);
    }}).catch(function (err) {{
      pollFailCount += 1;
      pollBackoffMs = Math.min(5000, Math.round(pollBackoffMs * 1.4));
      if (progressLabel) {{
        progressLabel.textContent = "Waiting for panel after restart (" + pollFailCount + ")...";
      }}
      if (pollFailCount >= POLL_FAIL_HARD_MAX) {{
        finishPollOk();
        if (opError) {{
          opError.textContent = friendlyFetchError(err, "Status poll") +
            " If the package already matches tip, refresh this page.";
        }}
        check(false);
        return;
      }}
      if (pollFailCount >= POLL_FAIL_SOFT_MAX && opError) {{
        opError.textContent = "Reconnecting to panel after restart...";
      }}
      schedulePoll(pollBackoffMs);
    }});
  }}
  function startJob(action, version) {{
    if (!canManage || busy) return;
    busy = true;
    setActionsEnabled(false);
    clearConfirm();
    pollFailCount = 0;
    pollBackoffMs = 500;
    if (opError) opError.textContent = "";
    if (progressWrap) progressWrap.style.display = "block";
    if (progressBar) progressBar.style.width = "1%";
    if (progressLabel) progressLabel.textContent = "1% Starting...";
    var installed = infoCache && infoCache.installed_version ? infoCache.installed_version : "";
    var isDown = action === "downgrade" || (version && installed && cmp(version, installed) < 0);
    var body = {{
      action: action,
      version: version || null,
      confirm_execute: true,
      confirm_downgrade: !!isDown,
      reset_data: false
    }};
    fetch("/api/maintenance", {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Accept": "application/json", "Content-Type": "application/json" }},
      body: JSON.stringify(body)
    }}).then(function (res) {{
      return res.json().then(function (data) {{
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      }});
    }}).then(function () {{
      schedulePoll(400);
    }}).catch(function (err) {{
      busy = false;
      setActionsEnabled(true);
      if (opError) opError.textContent = friendlyFetchError(err, "Start maintenance");
      if (progressLabel) progressLabel.textContent = "Not started.";
    }});
  }}
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
      armConfirm("upgrade", latest || null, "upgrade to latest" + (latest ? (" (" + latest + ")") : ""));
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
  check(false);
}})();
</script>"#,
        helpers = version_fetch_helpers_script(),
        can_manage_js = can_manage_js
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
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

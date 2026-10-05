//! Version page poll/reload JS: wait for panel listen after upgrade/repair, then reload.

/// Inline JS (same IIFE as Version Management). Reloads after a healthy listen.
pub fn version_page_poll_script() -> &'static str {
    r##"
  var pollFailCount = 0;
  var pollBackoffMs = 500;
  var POLL_FAIL_SOFT_MAX = 6;
  var POLL_FAIL_HARD_MAX = 60;
  var awaitingReconnect = false;
  var sawCompletedBeforeDisconnect = false;
  var sawDisconnect = false;
  var completedAt = 0;
  var reloadStarted = false;
  var RESTART_GRACE_MS = 8000;
  function schedulePoll(delayMs) {
    if (pollTimer) { clearTimeout(pollTimer); pollTimer = null; }
    pollTimer = setTimeout(pollStatus, Math.max(250, delayMs || 500));
  }
  function finishPollOk() {
    busy = false;
    setActionsEnabled(true);
    if (pollTimer) { clearTimeout(pollTimer); pollTimer = null; }
    pollFailCount = 0;
    pollBackoffMs = 500;
  }
  function versionReloadTarget() {
    var path = window.location.pathname || "/settings/version";
    if (path.indexOf("/settings/version") !== 0) {
      path = "/settings/version";
    }
    return path + (window.location.search || "");
  }
  function armMetaRefresh() {
    if (document.getElementById("cpn-version-reload-meta")) return;
    var meta = document.createElement("meta");
    meta.id = "cpn-version-reload-meta";
    meta.httpEquiv = "refresh";
    meta.content = "90;url=/settings/version";
    document.head.appendChild(meta);
  }
  function reloadWhenHealthy() {
    if (reloadStarted) return;
    reloadStarted = true;
    if (progressLabel) {
      progressLabel.textContent = "100% Completed. Reloading panel...";
    }
    window.location.replace(versionReloadTarget());
  }
  function shouldReloadNow() {
    if (sawDisconnect) return true;
    if (!sawCompletedBeforeDisconnect || !completedAt) return false;
    return (Date.now() - completedAt) >= RESTART_GRACE_MS;
  }
  function finishAfterReconnect(st) {
    finishPollOk();
    if (progressBar) progressBar.style.width = "100%";
    if (opError) opError.textContent = "";
    if (st && st.error) {
      if (opError) opError.textContent = st.error;
      if (progressLabel) progressLabel.textContent = "Failed after reconnect: " + st.error;
      awaitingReconnect = false;
      sawCompletedBeforeDisconnect = false;
      return;
    }
    if (progressLabel) {
      progressLabel.textContent = "100% Completed. Panel reconnected after restart. Reloading...";
    }
    reloadWhenHealthy();
  }
  function probeLoginThen(cont) {
    fetch("/login", {
      method: "GET",
      credentials: "same-origin",
      cache: "no-store",
      redirect: "manual"
    }).then(function (res) {
      if (res && (res.ok || res.status === 301 || res.status === 302 || res.status === 303 || res.type === "opaqueredirect")) {
        cont(true);
        return;
      }
      cont(false);
    }).catch(function () { cont(false); });
  }
  function pollStatus() {
    fetch("/api/maintenance/status", {
      credentials: "same-origin",
      headers: { "Accept": "application/json" },
      cache: "no-store"
    }).then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }).then(function (st) {
      pollFailCount = 0;
      pollBackoffMs = 500;
      if (opError && (opError.textContent.indexOf("Status poll failed") === 0 ||
          opError.textContent.indexOf("Reconnecting to panel") === 0)) {
        opError.textContent = "";
      }
      var pct = Math.max(0, Math.min(100, Math.round(Number(st.progress) || 0)));
      if (progressBar) progressBar.style.width = pct + "%";
      if (progressLabel) {
        var body = (st.phase || "") + (st.message ? (": " + st.message) : "");
        progressLabel.textContent = pct + "%" + (body ? (" " + body) : "");
      }
      if (st.phase === "completed") {
        sawCompletedBeforeDisconnect = true;
        if (!completedAt) completedAt = Date.now();
      }
      if (st.error && st.phase === "failed") {
        finishPollOk();
        if (opError) opError.textContent = st.error;
        if (progressLabel) progressLabel.textContent = pct + "% Failed: " + st.error;
        return;
      }
      var restartComing = !!st.restart_scheduled;
      if (awaitingReconnect && !st.busy && st.phase !== "completed" && shouldReloadNow()) {
        finishAfterReconnect(st);
        return;
      }
      if (!st.busy && st.phase === "completed") {
        awaitingReconnect = true;
        if (progressBar) progressBar.style.width = "100%";
        if (opError) opError.textContent = "";
        if (!restartComing) {
          if (progressLabel) {
            progressLabel.textContent = "100% Completed. Reloading panel...";
          }
          finishAfterReconnect(st);
          return;
        }
        if (progressLabel) {
          progressLabel.textContent = "100% Completed. Waiting for panel restart...";
        }
        if (shouldReloadNow()) {
          finishAfterReconnect(st);
          return;
        }
        schedulePoll(400);
        return;
      }
      if (!st.busy && (st.phase === "ready" || st.phase === "failed") && !awaitingReconnect) {
        finishPollOk();
        if (st.phase === "failed") {
          if (opError) opError.textContent = st.error || "Maintenance failed";
        }
        return;
      }
      schedulePoll(500);
    }).catch(function (err) {
      pollFailCount += 1;
      awaitingReconnect = true;
      sawDisconnect = true;
      pollBackoffMs = Math.min(4000, Math.round(pollBackoffMs * 1.35));
      if (progressLabel) {
        progressLabel.textContent = "Waiting for panel after restart (" + pollFailCount + ")...";
      }
      if (pollFailCount >= POLL_FAIL_HARD_MAX) {
        probeLoginThen(function (loginOk) {
          if (loginOk || sawCompletedBeforeDisconnect) {
            finishAfterReconnect({ phase: "completed", busy: false, error: null });
            return;
          }
          finishPollOk();
          if (opError) {
            opError.textContent = friendlyFetchError(err, "Status poll") +
              " Package apply may have finished. Confirm cpn-installer is active (sudo systemctl start cpn-installer; sudo cpn doctor --heal), then refresh this page on the same host you used (localhost vs 127.0.0.1).";
          }
          if (progressLabel) {
            progressLabel.textContent = "Reconnect timed out. Refresh /settings/version when the panel is back.";
          }
        });
        return;
      }
      if (pollFailCount >= POLL_FAIL_SOFT_MAX && opError) {
        opError.textContent = "Reconnecting to panel after restart on " + window.location.origin + "...";
      }
      if (pollFailCount >= POLL_FAIL_SOFT_MAX && pollFailCount % 3 === 0) {
        probeLoginThen(function (loginOk) {
          if (loginOk) {
            schedulePoll(400);
            return;
          }
          schedulePoll(pollBackoffMs);
        });
        return;
      }
      schedulePoll(pollBackoffMs);
    });
  }
  function startJob(action, version) {
    if (!canManage || busy) return;
    busy = true;
    setActionsEnabled(false);
    clearConfirm();
    pollFailCount = 0;
    pollBackoffMs = 500;
    awaitingReconnect = false;
    sawCompletedBeforeDisconnect = false;
    sawDisconnect = false;
    completedAt = 0;
    reloadStarted = false;
    armMetaRefresh();
    if (opError) opError.textContent = "";
    if (progressWrap) progressWrap.style.display = "block";
    if (progressBar) progressBar.style.width = "1%";
    if (progressLabel) progressLabel.textContent = "1% Starting...";
    var installed = infoCache && infoCache.installed_version ? infoCache.installed_version : "";
    var isDown = action === "downgrade" || (version && installed && cmp(version, installed) < 0);
    var body = {
      action: action,
      version: version || null,
      confirm_execute: true,
      confirm_downgrade: !!isDown,
      reset_data: false
    };
    fetch("/api/maintenance", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Accept": "application/json", "Content-Type": "application/json" },
      body: JSON.stringify(body)
    }).then(function (res) {
      return res.json().then(function (data) {
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      });
    }).then(function () {
      schedulePoll(400);
    }).catch(function (err) {
      busy = false;
      setActionsEnabled(true);
      if (opError) opError.textContent = friendlyFetchError(err, "Start maintenance");
      if (progressLabel) progressLabel.textContent = "Not started.";
    });
  }
"##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_script_reloads_after_healthy_listen() {
        let js = version_page_poll_script();
        assert!(js.contains("location.replace"));
        assert!(js.contains("Waiting for panel restart"));
        assert!(js.contains("Reloading panel"));
        assert!(js.contains("armMetaRefresh"));
        assert!(js.contains("sawDisconnect"));
        assert!(js.contains("shouldReloadNow"));
        assert!(js.contains("restart_scheduled"));
        assert!(js.contains("startJob"));
        assert!(js.contains("Repair") || js.contains("repair") || js.contains("action"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

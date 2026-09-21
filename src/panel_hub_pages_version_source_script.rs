//! Client script for Update source settings on Version Management.

/// Shared fetch error helper for Version Management pages.
pub fn version_fetch_helpers_script() -> &'static str {
    r##"<script>
window.cpnFriendlyFetchError = function (err, context) {
  var msg = err && err.message ? err.message : String(err || "unknown error");
  if (msg === "Failed to fetch" || msg.indexOf("NetworkError") >= 0 || msg.indexOf("Load failed") >= 0) {
    return (context || "Request") + " failed: network error (panel may be restarting, or the browser lost connection). Confirm cpn-installer is running, use the same host you signed in on (127.0.0.1 vs localhost use different cookies), then refresh.";
  }
  if (msg.indexOf("HTTP 401") >= 0) {
    return (context || "Request") + " failed: not signed in. Sign in again.";
  }
  if (msg.indexOf("HTTP 403") >= 0) {
    return (context || "Request") + " failed: permission denied for this action.";
  }
  if (msg.indexOf("HTTP 502") >= 0 || msg.indexOf("HTTP 503") >= 0 || msg.indexOf("HTTP 504") >= 0) {
    return (context || "Request") + " failed: panel service unavailable (" + msg + "). Try again shortly.";
  }
  return (context || "Request") + " failed: " + msg;
};
</script>"##
}

/// Inline `<script>` for fork/update source save UI. Admin only.
pub fn version_source_script() -> String {
    r##"<script>
(function () {
  var repoEl = document.getElementById("cpn-source-repo");
  var tokenEl = document.getElementById("cpn-source-token");
  var clearTokenEl = document.getElementById("cpn-source-clear-token");
  var saveBtn = document.getElementById("cpn-source-save");
  var statusEl = document.getElementById("cpn-source-status");
  if (!repoEl || !saveBtn) return;

  function sourceStatus(msg, isError) {
    if (!statusEl) return;
    statusEl.textContent = msg || "";
    statusEl.style.color = isError ? "#f87171" : "";
  }

  function loadSource() {
    fetch("/api/version-source", {
      credentials: "same-origin",
      headers: { "Accept": "application/json" }
    }).then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }).then(function (data) {
      if (data && data.repo) repoEl.value = data.repo;
      var tokenNote = data && data.token_configured ? "Token is configured on the server." : "No GitHub token stored (optional for public forks).";
      sourceStatus(tokenNote, false);
    }).catch(function (err) {
      sourceStatus(window.cpnFriendlyFetchError(err, "Load update source"), true);
    });
  }

  saveBtn.addEventListener("click", function () {
    saveBtn.disabled = true;
    sourceStatus("Saving update source...", false);
    var body = {
      repo: repoEl.value || "",
      github_token: tokenEl ? (tokenEl.value || "") : "",
      clear_token: !!(clearTokenEl && clearTokenEl.checked)
    };
    fetch("/api/version-source", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Accept": "application/json", "Content-Type": "application/json" },
      body: JSON.stringify(body)
    }).then(function (res) {
      return res.json().then(function (data) {
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      });
    }).then(function (data) {
      if (data && data.repo) repoEl.value = data.repo;
      if (tokenEl) tokenEl.value = "";
      if (clearTokenEl) clearTokenEl.checked = false;
      sourceStatus("Update source saved. Run Check for updates to refresh release lists.", false);
      if (typeof window.cpnVersionRecheck === "function") window.cpnVersionRecheck(true);
    }).catch(function (err) {
      sourceStatus(window.cpnFriendlyFetchError(err, "Save update source"), true);
    }).finally(function () {
      saveBtn.disabled = false;
    });
  });

  loadSource();
})();
</script>"##
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_script_has_api_paths() {
        let js = version_source_script();
        assert!(js.contains("/api/version-source"));
        assert!(version_fetch_helpers_script().contains("cpnFriendlyFetchError"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

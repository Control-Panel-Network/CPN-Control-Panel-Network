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
window.cpnVersionSetStatus = function (el, msg, kind) {
  if (!el) return;
  var k = kind || "neutral";
  el.textContent = msg || "";
  el.setAttribute("role", "status");
  if (k === "ok") {
    el.className = "panel-notice ok";
    el.style.marginTop = "10px";
  } else if (k === "error") {
    el.className = "panel-notice error";
    el.style.marginTop = "10px";
  } else {
    el.className = "muted";
    el.style.marginTop = "10px";
  }
};
window.cpnVersionFlashSave = function (title, body) {
  var wrap = document.getElementById("cpn-version-flash-anchor");
  if (!wrap) {
    var src = document.getElementById("cpn-source-status");
    var parent = src && src.parentElement;
    wrap = document.createElement("div");
    wrap.id = "cpn-version-flash-anchor";
    wrap.setAttribute("aria-live", "polite");
    if (parent && src) parent.insertBefore(wrap, src);
    else if (parent) parent.appendChild(wrap);
    else return;
  }
  wrap.innerHTML = '<p class="panel-notice ok" role="status"><strong>'
    + String(title || "Saved").replace(/&/g, "&amp;").replace(/</g, "&lt;")
    + "</strong>"
    + (body ? " " + String(body).replace(/&/g, "&amp;").replace(/</g, "&lt;") : "")
    + "</p>";
};
</script>"##
}

/// Inline `<script>` for fork/update source save UI (repo, branch, token). Admin only.
pub fn version_source_script() -> String {
    r##"<script>
(function () {
  var repoEl = document.getElementById("cpn-source-repo");
  var branchEl = document.getElementById("cpn-source-branch");
  var branchWarnEl = document.getElementById("cpn-source-branch-warning");
  var tokenEl = document.getElementById("cpn-source-token");
  var clearTokenEl = document.getElementById("cpn-source-clear-token");
  var saveBtn = document.getElementById("cpn-source-save");
  var statusEl = document.getElementById("cpn-source-status");
  if (!repoEl || !saveBtn) return;
  var DEFAULT_BRANCH = (branchEl && branchEl.getAttribute("data-default-branch")) || "stable";
  var savedBranch = DEFAULT_BRANCH;

  function sourceStatus(msg, kind) {
    if (typeof window.cpnVersionSetStatus === "function") {
      window.cpnVersionSetStatus(statusEl, msg, kind);
    } else if (statusEl) {
      statusEl.textContent = msg || "";
    }
  }

  function branchLabel(name) {
    if (name === "stable") return "stable (production: releases and stable commits)";
    if (name === "dev") return "dev (pre-release testing, lab only)";
    return name + " (other branch, lab only)";
  }

  function ensureBranchOption(name, label) {
    if (!branchEl || !name) return;
    for (var i = 0; i < branchEl.options.length; i++) {
      if (branchEl.options[i].value === name) {
        if (label) branchEl.options[i].textContent = label;
        return;
      }
    }
    var opt = document.createElement("option");
    opt.value = name;
    opt.textContent = label || branchLabel(name);
    branchEl.appendChild(opt);
  }

  function selectBranch(name) {
    if (!branchEl) return;
    var want = name || DEFAULT_BRANCH;
    ensureBranchOption(want, branchLabel(want));
    branchEl.value = want;
    paintBranchWarning();
  }

  function paintBranchWarning() {
    if (!branchEl || !branchWarnEl) return;
    var current = branchEl.value || DEFAULT_BRANCH;
    if (current === DEFAULT_BRANCH) {
      branchWarnEl.style.display = "none";
      branchWarnEl.textContent = "";
      return;
    }
    var text = current === "dev"
      ? "dev is for lab and pre-release testing. Production servers should stay on stable. Upgrade to latest commits will follow dev after you save."
      : ("Branch " + current + " is not a production branch (lab only). Production servers should stay on stable.");
    if (current !== savedBranch) text += " Not saved yet: click Save update source.";
    branchWarnEl.textContent = text;
    branchWarnEl.style.display = "block";
  }

  function loadBranches() {
    if (!branchEl) return;
    fetch("/api/version-branches", {
      credentials: "same-origin",
      headers: { "Accept": "application/json" }
    }).then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }).then(function (data) {
      var list = (data && data.branches) || [];
      var keep = branchEl.value || savedBranch;
      for (var i = 0; i < list.length; i++) {
        var item = list[i] || {};
        if (!item.name) continue;
        ensureBranchOption(item.name, item.label || branchLabel(item.name));
      }
      selectBranch(keep);
      if (data && data.error && statusEl && !statusEl.textContent) {
        sourceStatus("Branch list from GitHub unavailable (" + data.error + "); stable and dev are always offered.", "neutral");
      }
    }).catch(function () {
      // Known branches (stable, dev) are already in the select; GitHub list is optional.
    });
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
      savedBranch = (data && data.branch) || DEFAULT_BRANCH;
      selectBranch(savedBranch);
      var tokenNote = data && data.token_configured ? "Token is configured on the server." : "No GitHub token stored (optional for public forks).";
      var branchNote = "Commit branch: " + savedBranch + (savedBranch === DEFAULT_BRANCH ? " (production)." : " (lab / pre-release testing).");
      sourceStatus(branchNote + " " + tokenNote, "neutral");
      loadBranches();
    }).catch(function (err) {
      sourceStatus(window.cpnFriendlyFetchError(err, "Load update source"), "error");
      loadBranches();
    });
  }

  if (branchEl) branchEl.addEventListener("change", paintBranchWarning);

  saveBtn.addEventListener("click", function () {
    saveBtn.disabled = true;
    sourceStatus("Saving update source...", "neutral");
    var branch = branchEl ? (branchEl.value || DEFAULT_BRANCH) : DEFAULT_BRANCH;
    var body = {
      repo: repoEl.value || "",
      branch: branch,
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
      savedBranch = (data && data.branch) || branch;
      selectBranch(savedBranch);
      if (tokenEl) tokenEl.value = "";
      if (clearTokenEl) clearTokenEl.checked = false;
      var tail = savedBranch === DEFAULT_BRANCH
        ? "Upgrade to latest commits follows stable."
        : ("Upgrade to latest commits now follows " + savedBranch + " (lab / pre-release testing).");
      sourceStatus("Update source saved. " + tail + " Checking for updates...", "ok");
      if (typeof window.cpnVersionFlashSave === "function") {
        window.cpnVersionFlashSave("Update source saved", tail);
      }
      if (typeof window.cpnVersionRecheck === "function") window.cpnVersionRecheck(true);
    }).catch(function (err) {
      sourceStatus(window.cpnFriendlyFetchError(err, "Save update source"), "error");
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
        assert!(js.contains("/api/version-branches"));
        assert!(js.contains("cpn-source-branch"));
        assert!(js.contains("branch: branch"));
        assert!(js.contains("Production servers should stay on stable"));
        assert!(js.contains("cpnVersionRecheck"));
        assert!(version_fetch_helpers_script().contains("cpnFriendlyFetchError"));
        assert!(version_fetch_helpers_script().contains("cpnVersionSetStatus"));
        assert!(version_fetch_helpers_script().contains("panel-notice ok"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

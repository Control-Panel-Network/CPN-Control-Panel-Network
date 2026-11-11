//! Client script for Automatic updates settings on Version Management.

/// Inline `<script>` for Owner automatic-update toggles. Admin only.
pub fn version_auto_upgrade_script() -> String {
    r##"<script>
(function () {
  var enabledEl = document.getElementById("cpn-auto-enabled");
  var modeEl = document.getElementById("cpn-auto-mode");
  var preEl = document.getElementById("cpn-auto-prerelease");
  var saveBtn = document.getElementById("cpn-auto-save");
  var checkBtn = document.getElementById("cpn-auto-check");
  var statusEl = document.getElementById("cpn-auto-status");
  var lastEl = document.getElementById("cpn-auto-last");
  var noteEl = document.getElementById("cpn-auto-security-note");
  if (!enabledEl || !saveBtn) return;

  function autoStatus(msg, kind) {
    if (typeof window.cpnVersionSetStatus === "function") {
      window.cpnVersionSetStatus(statusEl, msg, kind);
    } else if (statusEl) {
      statusEl.textContent = msg || "";
    }
  }

  function paint(data) {
    if (!data) return;
    enabledEl.checked = !!data.enabled;
    if (modeEl) modeEl.value = (data.mode === "security") ? "security" : "all";
    if (preEl) preEl.checked = !!data.include_prerelease;
    if (noteEl && data.security_detection_note) noteEl.textContent = data.security_detection_note;
    if (lastEl) {
      var bits = [];
      if (data.last_result) bits.push(data.last_result);
      if (data.last_applied_version) bits.push("Last applied: " + data.last_applied_version);
      lastEl.textContent = bits.join(" ");
    }
  }

  function loadAuto() {
    fetch("/api/auto-upgrade", {
      credentials: "same-origin",
      headers: { "Accept": "application/json" }
    }).then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }).then(paint).catch(function (err) {
      autoStatus(window.cpnFriendlyFetchError(err, "Load automatic updates"), "error");
    });
  }

  saveBtn.addEventListener("click", function () {
    saveBtn.disabled = true;
    autoStatus("Saving...", "neutral");
    var body = {
      enabled: !!enabledEl.checked,
      mode: modeEl ? modeEl.value : "all",
      include_prerelease: !!(preEl && preEl.checked)
    };
    fetch("/api/auto-upgrade", {
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
      paint(data);
      autoStatus("Automatic updates saved.", "ok");
      if (typeof window.cpnVersionFlashSave === "function") {
        var detail = data.enabled
          ? "Scheduled release checks stay on."
          : "Background upgrades stay off.";
        window.cpnVersionFlashSave("Automatic updates saved", detail);
      }
    }).catch(function (err) {
      autoStatus(window.cpnFriendlyFetchError(err, "Save automatic updates"), "error");
    }).finally(function () {
      saveBtn.disabled = false;
    });
  });

  if (checkBtn) {
    checkBtn.addEventListener("click", function () {
      checkBtn.disabled = true;
      autoStatus("Checking for updates (dry-run, will not apply while off)...", "neutral");
      fetch("/api/auto-upgrade/check", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Accept": "application/json", "Content-Type": "application/json" },
        body: JSON.stringify({ apply: false })
      }).then(function (res) {
        return res.json().then(function (data) {
          if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
          return data;
        });
      }).then(function (data) {
        autoStatus((data && data.message) || "Check complete.", "ok");
        if (lastEl && data && data.message) lastEl.textContent = data.message;
      }).catch(function (err) {
        autoStatus(window.cpnFriendlyFetchError(err, "Check automatic updates"), "error");
      }).finally(function () {
        checkBtn.disabled = false;
      });
    });
  }

  loadAuto();
})();
</script>"##
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_script_wires_apis() {
        let js = version_auto_upgrade_script();
        assert!(js.contains("/api/auto-upgrade"));
        assert!(js.contains("/api/auto-upgrade/check"));
        assert!(js.contains("cpn-auto-enabled"));
        assert!(js.contains("apply: false"));
        assert!(js.contains("cpnVersionFlashSave"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
        assert!(!js.to_lowercase().contains("cyberpanel"));
    }
}

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
  var countdownEl = document.getElementById("cpn-auto-countdown");
  if (!enabledEl || !saveBtn) return;

  var scheduleState = {
    nextAt: 0,
    show: false,
    jitter: 0,
    baseInterval: 0
  };
  var countdownTimer = null;

  function autoStatus(msg, kind) {
    if (typeof window.cpnVersionSetStatus === "function") {
      window.cpnVersionSetStatus(statusEl, msg, kind);
    } else if (statusEl) {
      statusEl.textContent = msg || "";
    }
  }

  function pad2(n) {
    return String(n).padStart(2, "0");
  }

  function formatLocalDateTime(unix) {
    var ts = Number(unix);
    if (!ts || !isFinite(ts) || ts <= 0) return "";
    var d = new Date(ts * 1000);
    if (isNaN(d.getTime())) return "";
    return pad2(d.getDate()) + "/" + pad2(d.getMonth() + 1) + "/" + d.getFullYear()
      + " " + pad2(d.getHours()) + ":" + pad2(d.getMinutes());
  }

  function formatCountdown(totalSecs) {
    var s = Math.max(0, Math.floor(Number(totalSecs) || 0));
    var h = Math.floor(s / 3600);
    var m = Math.floor((s % 3600) / 60);
    var sec = s % 60;
    if (h > 0) {
      return h + "h " + m + "m " + sec + "s";
    }
    if (m > 0) {
      return m + "m " + sec + "s";
    }
    return sec + "s";
  }

  function paintCountdown() {
    if (!countdownEl) return;
    if (!scheduleState.show || !scheduleState.nextAt) {
      countdownEl.style.display = "none";
      countdownEl.textContent = "";
      return;
    }
    var now = Math.floor(Date.now() / 1000);
    var remain = Math.max(0, scheduleState.nextAt - now);
    var when = formatLocalDateTime(scheduleState.nextAt);
    var jitterNote = scheduleState.jitter
      ? " (includes " + scheduleState.jitter + "s per-install jitter on "
        + scheduleState.baseInterval + "s base interval)"
      : "";
    countdownEl.style.display = "block";
    countdownEl.textContent = "Next automatic update check in "
      + formatCountdown(remain)
      + (when ? " (about " + when + " local)" : "")
      + ". Checks are dry until a newer installable release exists."
      + jitterNote
      + " Hide this on Settings overview.";
  }

  function scheduleCountdownTick() {
    if (countdownTimer) {
      clearInterval(countdownTimer);
      countdownTimer = null;
    }
    paintCountdown();
    if (scheduleState.show && scheduleState.nextAt) {
      countdownTimer = setInterval(paintCountdown, 1000);
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
    var nextAt = Number(data.next_check_at_unix) || 0;
    if (nextAt <= 0 && data.seconds_until_next_check) {
      nextAt = Math.floor(Date.now() / 1000) + Number(data.seconds_until_next_check);
    }
    scheduleState.nextAt = nextAt;
    scheduleState.show = !!(data.show_countdown && data.enabled);
    scheduleState.jitter = Number(data.schedule_jitter_secs) || 0;
    scheduleState.baseInterval = Number(data.check_interval_base_secs) || Number(data.check_interval_secs) || 0;
    scheduleCountdownTick();
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
        loadAuto();
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
        assert!(js.contains("cpn-auto-countdown"));
        assert!(js.contains("apply: false"));
        assert!(js.contains("cpnVersionFlashSave"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
        assert!(!js.to_lowercase().contains("cyberpanel"));
    }
}

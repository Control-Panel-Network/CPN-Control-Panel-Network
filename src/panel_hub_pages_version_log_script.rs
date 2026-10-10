//! Version page installer-log JS: local timestamps, done/failed state, copy to clipboard.
//!
//! Session lines arrive as `[dd/mm/yyyy HH:MM:SS] <marker> <message>` with the
//! server UTC clock; the browser rewrites the stamp to its local time in the same
//! Norwegian/European `dd/mm/yyyy HH:MM:SS` (24h) layout.

/// Inline JS (same IIFE as Version Management). Defines `paintInstallerLog`.
pub fn version_page_log_script() -> &'static str {
    r##"
  var LOG_STAMP_RE = /^\[(\d{2})\/(\d{2})\/(\d{4}) (\d{2}):(\d{2}):(\d{2})\] ?/;
  function pad2(n) { return (n < 10 ? "0" : "") + n; }
  function localStampFromUtcMatch(m) {
    var d = new Date(Date.UTC(+m[3], +m[2] - 1, +m[1], +m[4], +m[5], +m[6]));
    if (isNaN(d.getTime())) return m[0];
    return pad2(d.getDate()) + "/" + pad2(d.getMonth() + 1) + "/" + d.getFullYear() + " " +
      pad2(d.getHours()) + ":" + pad2(d.getMinutes()) + ":" + pad2(d.getSeconds());
  }
  function classifyLogLine(body) {
    if (body.indexOf("DONE ") === 0) return "ssh-done";
    if (body.indexOf("FAILED (job)") === 0) return "ssh-fail ssh-done";
    if (body.indexOf("note ") === 0) return "ssh-note";
    if (body.indexOf("FAILED ") === 0) return "ssh-fail";
    if (/\[error\]/.test(body)) return "ssh-fail";
    return "";
  }
  function setLogState(state, text) {
    var el = document.getElementById("cpn-version-log-state");
    if (!el) return;
    if (!state) {
      el.removeAttribute("data-state");
      el.textContent = "";
      return;
    }
    el.setAttribute("data-state", state);
    el.textContent = text;
  }
  function deriveLogState(lines, st) {
    var last = "";
    for (var i = lines.length - 1; i >= 0; i--) {
      var body = lines[i].replace(LOG_STAMP_RE, "");
      if (body.indexOf("note ") === 0) continue;
      last = body;
      break;
    }
    if (st && st.busy) return ["running", "running"];
    if (last.indexOf("DONE ") === 0) return ["done", "done"];
    if (last.indexOf("FAILED (job)") === 0) return ["failed", "failed"];
    if (st && st.phase === "failed") return ["failed", "failed"];
    if (st && st.phase === "completed") return ["done", "done"];
    return ["", ""];
  }
  function paintInstallerLog(text, forceOpen, st) {
    var box = document.getElementById("cpn-version-log");
    var pre = document.getElementById("cpn-version-log-pre");
    if (!pre) return;
    var raw = String(text || "");
    pre.innerHTML = "";
    if (!raw) {
      pre.textContent = "";
      setLogState("", "");
      return;
    }
    var lines = raw.split("\n");
    lines.forEach(function (line) {
      var m = LOG_STAMP_RE.exec(line);
      var body = m ? line.slice(m[0].length) : line;
      var row = document.createElement("span");
      var cls = classifyLogLine(body);
      if (cls) row.className = cls;
      if (m) {
        var t = document.createElement("span");
        t.className = "ssh-time";
        t.textContent = "[" + localStampFromUtcMatch(m) + "] ";
        t.title = "Server time (UTC): " + m[0].replace(/^\[|\] ?$/g, "");
        row.appendChild(t);
      }
      row.appendChild(document.createTextNode(body + "\n"));
      pre.appendChild(row);
    });
    var state = deriveLogState(lines, st);
    setLogState(state[0], state[1]);
    pre.scrollTop = pre.scrollHeight;
    if (box && forceOpen) box.open = true;
  }
  function copyInstallerLog() {
    var pre = document.getElementById("cpn-version-log-pre");
    var btn = document.getElementById("cpn-version-log-copy");
    if (!pre) return;
    var text = pre.textContent || "";
    function flash(ok) {
      if (!btn) return;
      var original = "Copy log";
      btn.textContent = ok ? "Copied" : "Copy failed";
      if (ok) btn.setAttribute("data-copied", "1");
      setTimeout(function () {
        btn.textContent = original;
        btn.removeAttribute("data-copied");
      }, 1800);
    }
    function legacyCopy() {
      var ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.left = "-9999px";
      document.body.appendChild(ta);
      ta.select();
      var ok = false;
      try { ok = document.execCommand("copy"); } catch (e) { ok = false; }
      document.body.removeChild(ta);
      flash(ok);
    }
    if (navigator.clipboard && navigator.clipboard.writeText && window.isSecureContext) {
      navigator.clipboard.writeText(text).then(function () { flash(true); }, legacyCopy);
      return;
    }
    legacyCopy();
  }
  (function () {
    var copyBtn = document.getElementById("cpn-version-log-copy");
    if (copyBtn) copyBtn.addEventListener("click", copyInstallerLog);
  })();
"##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_script_has_timestamps_copy_and_done_state() {
        let js = version_page_log_script();
        assert!(js.contains("LOG_STAMP_RE"));
        assert!(js.contains("Date.UTC"));
        assert!(js.contains(
            "pad2(d.getDate()) + \"/\" + pad2(d.getMonth() + 1) + \"/\" + d.getFullYear()"
        ));
        assert!(js.contains("navigator.clipboard"));
        assert!(js.contains("execCommand(\"copy\")"));
        assert!(js.contains("cpn-version-log-copy"));
        assert!(js.contains("ssh-done"));
        assert!(js.contains("ssh-note"));
        assert!(js.contains("DONE "));
        assert!(js.contains("FAILED (job)"));
        assert!(js.contains("function paintInstallerLog(text, forceOpen, st)"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

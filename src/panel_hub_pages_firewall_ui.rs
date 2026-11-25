//! Shared styles, dialogs and client script for the firewall manager page.
//!
//! The three tabs are rendered into one page and switched on the client, so changing
//! tabs never reloads the page or re-runs the host firewall probes. Every action is
//! still a real `<form method="post">`, so the page degrades to plain navigation
//! without JavaScript (tab links keep their `?tab=` URLs).

use crate::panel_hub_pages_firewall::html_escape;

/// Accurate wording for the IP / CIDR field of an allow rule.
pub const IP_CIDR_TIP: &str = "The source address allowed to reach this port. \
0.0.0.0/0 means all sources: anyone on the internet can connect to the port \
(::/0 is the same for IPv6, and leaving the field empty also means all sources). \
Enter a single address such as 203.0.113.5, or a range such as 203.0.113.0/24, \
to allow only those sources.";

/// Protocol `<option>` list shared by the add form and the edit dialog.
pub const PROTOCOL_OPTIONS: &str = r#"<option value="tcp">TCP</option><option value="udp">UDP</option><option value="both">Both (TCP and UDP)</option>"#;

/// Small info button with a tooltip. `id` must be unique on the page.
pub fn info_tip(id: &str, label: &str, text: &str) -> String {
    format!(
        r#"<button type="button" class="fw-tip" aria-label="{label}" aria-describedby="{id}"><span aria-hidden="true">i</span><span class="fw-tip-body" role="tooltip" id="{id}">{text}</span></button>"#,
        id = html_escape(id),
        label = html_escape(label),
        text = html_escape(text),
    )
}

/// Page CSS (scoped to `#fw-root` and the firewall dialogs).
pub fn firewall_css() -> &'static str {
    r##"<style>
#fw-root .fw-tabs{display:flex;gap:8px;flex-wrap:wrap;margin:16px 0;}
#fw-root .fw-tab{display:inline-flex;align-items:center;min-height:36px;padding:0 14px;border-radius:8px;border:1px solid var(--hairline,#334155);background:transparent;color:inherit;font-weight:700;text-decoration:none;cursor:pointer;}
#fw-root .fw-tab.is-active{background:var(--accent,#7c3aed);color:#fff;border-color:transparent;}
#fw-root .fw-tab:focus-visible{outline:2px solid var(--blue,#2e90fa);outline-offset:2px;}
#fw-root [data-fw-panel][hidden]{display:none;}
#fw-root .fw-controls{display:flex;gap:8px;flex-wrap:wrap;margin-top:12px;}
.fw-btn{display:inline-flex;align-items:center;justify-content:center;min-height:40px;padding:0 22px;border-radius:999px;border:0;font-weight:700;font-size:inherit;color:#fff;cursor:pointer;}
.fw-btn:hover{filter:brightness(1.08);}
.fw-btn:focus-visible{outline:2px solid #fff;outline-offset:2px;box-shadow:0 0 0 4px rgba(46,144,250,.6);}
.fw-btn-start{background:#12b76a;}
.fw-btn-stop{background:#d92d20;}
.fw-btn-reload{background:#2e90fa;}
.fw-btn-sm{min-height:32px;padding:0 14px;font-size:13px;}
#fw-root .fw-actions{display:flex;gap:8px;flex-wrap:wrap;align-items:center;}
#fw-root .fw-actions form{margin:0;display:inline;}
#fw-root .fw-form{display:grid;gap:10px;align-items:end;margin:12px 0;padding:12px;border:1px solid var(--hairline,#334155);border-radius:10px;max-width:none;}
#fw-root .fw-field-head,.fw-dialog .fw-field-head{display:flex;align-items:center;gap:4px;margin-bottom:4px;}
#fw-root .fw-field label,.fw-dialog .fw-field label{display:inline;}
#fw-root .fw-field input,#fw-root .fw-field select,.fw-dialog .fw-field input,.fw-dialog .fw-field select{width:100%;box-sizing:border-box;}
.fw-tip{position:relative;display:inline-flex;align-items:center;justify-content:center;width:18px;height:18px;padding:0;border-radius:50%;border:1px solid var(--hairline,#64748b);background:transparent;color:var(--muted,#94a3b8);font-size:12px;font-weight:700;font-style:italic;cursor:help;}
.fw-tip:focus-visible{outline:2px solid var(--blue,#2e90fa);outline-offset:2px;}
.fw-tip-body{display:none;position:absolute;z-index:40;left:-8px;bottom:calc(100% + 8px);width:min(300px,72vw);padding:10px 12px;border-radius:10px;border:1px solid var(--hairline,#334155);background:var(--canvas,#0f172a);color:var(--ink,#e2e8f0);box-shadow:0 10px 30px rgba(0,0,0,.35);font-size:13px;font-weight:400;font-style:normal;line-height:1.45;text-align:left;}
.fw-tip:hover .fw-tip-body,.fw-tip:focus .fw-tip-body,.fw-tip:focus-within .fw-tip-body{display:block;}
.fw-dialog{overflow:visible;max-width:min(460px,92vw);width:100%;}
.fw-dialog .fw-dialog-form{padding:20px;margin:0;max-width:none;}
.fw-dialog-actions{display:flex;gap:8px;justify-content:flex-end;margin-top:12px;}
.fw-confirm-btn{background:#d92d20;color:#fff;border:0;}
#fw-root .fw-nomatch[hidden]{display:none;}
</style>"##
}

/// Delete / remove confirmation dialog. Cancel gets focus by default.
pub fn confirm_dialog_html() -> &'static str {
    r#"<dialog id="fw-confirm" class="cpn-modal fw-dialog" aria-labelledby="fw-confirm-title" aria-describedby="fw-confirm-desc">
  <div class="stack-form fw-dialog-form">
    <h2 id="fw-confirm-title">Delete entry?</h2>
    <p id="fw-confirm-desc" class="muted" style="margin:0 0 4px;"></p>
    <div class="fw-dialog-actions">
      <button type="button" class="btn-secondary" id="fw-confirm-cancel" autofocus>Cancel</button>
      <button type="button" class="btn-primary fw-confirm-btn" id="fw-confirm-ok">Delete</button>
    </div>
  </div>
</dialog>"#
}

/// Edit dialog for a firewall rule.
pub fn rule_edit_dialog_html(csrf: &str) -> String {
    format!(
        r#"<dialog id="fw-edit-rules" class="cpn-modal fw-dialog" aria-labelledby="fw-edit-rules-title">
  <form method="post" action="/security/firewall/rules/update" class="stack-form fw-dialog-form">
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="id" value="">
    <h2 id="fw-edit-rules-title">Edit firewall rule</h2>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-rules-name">Rule name</label></div><input id="fw-edit-rules-name" name="name" required maxlength="64"></div>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-rules-protocol">Protocol</label></div><select id="fw-edit-rules-protocol" name="protocol">{protocols}</select></div>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-rules-source">IP / CIDR</label>{tip}</div><input id="fw-edit-rules-source" name="source" maxlength="64" placeholder="0.0.0.0/0"></div>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-rules-port">Port</label></div><input id="fw-edit-rules-port" name="port" required maxlength="11" placeholder="80 or 8000-8100"></div>
    <div class="fw-dialog-actions"><button type="button" class="btn-secondary" data-fw-close>Cancel</button><button type="submit" class="btn-primary">Save</button></div>
  </form>
</dialog>"#,
        csrf = html_escape(csrf),
        protocols = PROTOCOL_OPTIONS,
        tip = info_tip("fw-tip-edit-source", "About IP / CIDR", IP_CIDR_TIP),
    )
}

/// Edit dialog for a banned IP.
pub fn ban_edit_dialog_html(csrf: &str) -> String {
    format!(
        r#"<dialog id="fw-edit-banned" class="cpn-modal fw-dialog" aria-labelledby="fw-edit-banned-title">
  <form method="post" action="/security/firewall/banned/update" class="stack-form fw-dialog-form">
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="old_ip" value="">
    <h2 id="fw-edit-banned-title">Edit banned IP</h2>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-banned-ip">IP address</label></div><input id="fw-edit-banned-ip" name="ip" required maxlength="64"></div>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-banned-reason">Reason</label></div><input id="fw-edit-banned-reason" name="reason" maxlength="200"></div>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-banned-duration">Duration</label></div><select id="fw-edit-banned-duration" name="duration">
      <option value="keep" selected>Keep current expiry</option>
      <option value="3600">1 Hour from now</option>
      <option value="86400">24 Hours from now</option>
      <option value="604800">7 Days from now</option>
      <option value="0">Never</option>
    </select></div>
    <div class="fw-dialog-actions"><button type="button" class="btn-secondary" data-fw-close>Cancel</button><button type="submit" class="btn-primary">Save</button></div>
  </form>
</dialog>"#,
        csrf = html_escape(csrf),
    )
}

/// Edit dialog for a manually added trusted IP.
pub fn trusted_edit_dialog_html(csrf: &str) -> String {
    format!(
        r#"<dialog id="fw-edit-trusted" class="cpn-modal fw-dialog" aria-labelledby="fw-edit-trusted-title">
  <form method="post" action="/security/firewall/trusted/update" class="stack-form fw-dialog-form">
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="old_ip" value="">
    <h2 id="fw-edit-trusted-title">Edit trusted IP</h2>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-trusted-ip">IP address</label></div><input id="fw-edit-trusted-ip" name="ip" required maxlength="64"></div>
    <div class="fw-field"><div class="fw-field-head"><label for="fw-edit-trusted-label">Label</label></div><input id="fw-edit-trusted-label" name="label" maxlength="64"></div>
    <div class="fw-dialog-actions"><button type="button" class="btn-secondary" data-fw-close>Cancel</button><button type="submit" class="btn-primary">Save</button></div>
  </form>
</dialog>"#,
        csrf = html_escape(csrf),
    )
}

/// Client script: instant tab switching, edit dialogs, delete confirmation, search.
pub fn firewall_script() -> &'static str {
    r##"<script>
(function () {
  if (window.__cpnFwBound) return;
  window.__cpnFwBound = true;
  var root = document.getElementById("fw-root");
  if (!root) return;
  var tabs = [].slice.call(root.querySelectorAll("[data-fw-tab]"));
  var panels = [].slice.call(root.querySelectorAll("[data-fw-panel]"));

  function showTab(id, remember) {
    var known = panels.some(function (p) { return p.getAttribute("data-fw-panel") === id; });
    if (!known) return false;
    panels.forEach(function (p) { p.hidden = p.getAttribute("data-fw-panel") !== id; });
    tabs.forEach(function (t) {
      var on = t.getAttribute("data-fw-tab") === id;
      t.classList.toggle("is-active", on);
      t.setAttribute("aria-selected", on ? "true" : "false");
      t.tabIndex = on ? 0 : -1;
    });
    if (remember && window.history && history.replaceState && window.URL) {
      var u = new URL(window.location.href);
      u.searchParams.set("tab", id);
      history.replaceState(null, "", u.pathname + u.search);
    }
    return true;
  }

  root.addEventListener("click", function (ev) {
    var tab = ev.target.closest ? ev.target.closest("[data-fw-tab]") : null;
    if (tab && root.contains(tab)) {
      if (ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.button) return;
      if (showTab(tab.getAttribute("data-fw-tab"), true)) ev.preventDefault();
    }
  });
  root.addEventListener("keydown", function (ev) {
    var tab = ev.target.closest ? ev.target.closest("[data-fw-tab]") : null;
    if (!tab || (ev.key !== "ArrowRight" && ev.key !== "ArrowLeft")) return;
    var i = tabs.indexOf(tab);
    var next = tabs[(i + (ev.key === "ArrowRight" ? 1 : tabs.length - 1)) % tabs.length];
    if (next && showTab(next.getAttribute("data-fw-tab"), true)) { next.focus(); ev.preventDefault(); }
  });

  function openDialog(dlg) {
    if (!dlg) return false;
    if (typeof dlg.showModal === "function") { dlg.showModal(); } else { dlg.setAttribute("open", ""); }
    return true;
  }
  function closeDialog(dlg) {
    if (!dlg) return;
    if (typeof dlg.close === "function") { dlg.close(); } else { dlg.removeAttribute("open"); }
  }

  /* Edit dialogs: data-field-<name> attributes on the Edit button fill form fields. */
  document.addEventListener("click", function (ev) {
    var btn = ev.target.closest ? ev.target.closest("[data-fw-edit]") : null;
    if (!btn) return;
    var dlg = document.getElementById("fw-edit-" + btn.getAttribute("data-fw-edit"));
    if (!dlg) return;
    var form = dlg.querySelector("form");
    Object.keys(btn.dataset).forEach(function (key) {
      if (key.indexOf("field") !== 0 || key.length <= 5) return;
      var name = key.charAt(5).toLowerCase() + key.slice(6);
      var el = form.elements[name];
      if (el) el.value = btn.dataset[key];
    });
    [].forEach.call(form.querySelectorAll("button[type=submit]"), function (b) { b.disabled = false; });
    openDialog(dlg);
    var first = form.querySelector("input[type=text], input:not([type])");
    if (first && first.focus) first.focus();
  });
  document.addEventListener("click", function (ev) {
    var t = ev.target;
    if (!t || !t.closest) return;
    if (t.closest("[data-fw-close]")) { closeDialog(t.closest("dialog")); return; }
    if (t.tagName === "DIALOG" && t.classList.contains("fw-dialog") && t.id !== "fw-confirm") closeDialog(t);
  });
  document.addEventListener("submit", function (ev) {
    var f = ev.target;
    if (!f || !f.classList || !f.classList.contains("fw-dialog-form")) return;
    setTimeout(function () {
      [].forEach.call(f.querySelectorAll("button[type=submit]"), function (b) { b.disabled = true; });
    }, 0);
  });

  /* Delete confirmation (Cancel is focused by default). */
  var pending = null;
  function confirmDlg() { return document.getElementById("fw-confirm"); }
  document.addEventListener("submit", function (ev) {
    var form = ev.target;
    if (!form || !form.hasAttribute || !form.hasAttribute("data-fw-confirm")) return;
    if (form.getAttribute("data-fw-confirmed") === "1") return;
    ev.preventDefault();
    var message = form.getAttribute("data-fw-confirm") || "This cannot be undone.";
    var title = form.getAttribute("data-fw-title") || "Delete entry?";
    var label = form.getAttribute("data-fw-label") || "Delete";
    var dlg = confirmDlg();
    if (dlg && typeof dlg.showModal === "function") {
      pending = form;
      document.getElementById("fw-confirm-title").textContent = title;
      document.getElementById("fw-confirm-desc").textContent = message;
      document.getElementById("fw-confirm-ok").textContent = label;
      dlg.showModal();
      var cancel = document.getElementById("fw-confirm-cancel");
      if (cancel) cancel.focus();
      return;
    }
    if (window.confirm(title + " " + message)) {
      form.setAttribute("data-fw-confirmed", "1");
      form.submit();
    }
  }, true);
  document.addEventListener("click", function (ev) {
    var t = ev.target;
    if (!t || !t.id) return;
    var dlg = confirmDlg();
    if (t.id === "fw-confirm-cancel") {
      pending = null;
      closeDialog(dlg);
    } else if (t.id === "fw-confirm-ok") {
      var form = pending;
      pending = null;
      closeDialog(dlg);
      if (form) { form.setAttribute("data-fw-confirmed", "1"); form.submit(); }
    }
  });
  document.addEventListener("close", function (ev) {
    if (ev.target && ev.target.id === "fw-confirm") pending = null;
  }, true);

  /* Banned IP search runs on the client; the GET form stays as the no-script fallback. */
  var search = root.querySelector("[data-fw-search-input]");
  if (search) {
    var rows = [].slice.call(root.querySelectorAll("tr[data-fw-search]"));
    var none = root.querySelector("[data-fw-nomatch]");
    var apply = function () {
      var q = search.value.trim().toLowerCase();
      var shown = 0;
      rows.forEach(function (r) {
        var hit = !q || r.getAttribute("data-fw-search").indexOf(q) !== -1;
        r.hidden = !hit;
        if (hit) shown++;
      });
      if (none) none.hidden = shown !== 0 || rows.length === 0;
    };
    search.addEventListener("input", apply);
    var sform = search.form;
    if (sform) sform.addEventListener("submit", function (ev) { ev.preventDefault(); apply(); });
    apply();
  }
})();
</script>"##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_explains_all_sources_and_is_escaped() {
        let html = info_tip("t<1>", "About \"x\"", IP_CIDR_TIP);
        assert!(html.contains("0.0.0.0/0 means all sources"));
        assert!(html.contains("role=\"tooltip\""));
        assert!(!html.contains("t<1>"));
    }

    #[test]
    fn protocol_options_include_both() {
        assert!(PROTOCOL_OPTIONS.contains(r#"value="both""#));
        assert!(PROTOCOL_OPTIONS.contains(r#"value="tcp""#));
        assert!(PROTOCOL_OPTIONS.contains(r#"value="udp""#));
    }

    #[test]
    fn confirm_dialog_focuses_cancel_and_edit_dialogs_post() {
        assert!(confirm_dialog_html().contains("id=\"fw-confirm-cancel\" autofocus"));
        assert!(firewall_script().contains("cancel.focus()"));
        assert!(rule_edit_dialog_html("tok").contains("/security/firewall/rules/update"));
        assert!(ban_edit_dialog_html("tok").contains("/security/firewall/banned/update"));
        assert!(trusted_edit_dialog_html("tok").contains("/security/firewall/trusted/update"));
    }

    #[test]
    fn assets_have_no_em_dash() {
        for s in [
            firewall_css(),
            firewall_script(),
            confirm_dialog_html(),
            IP_CIDR_TIP,
        ] {
            assert!(!s.contains('\u{2014}'));
        }
    }
}

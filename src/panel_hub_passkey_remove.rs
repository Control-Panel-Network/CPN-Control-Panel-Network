//! Confirmation dialog markup for removing a passkey (WebAuthn credential) on
//! the Security tab of Modify User. The same renderer serves the signed-in
//! user and the administrator view of Modify User, so one dialog covers both.
//!
//! Progressive enhancement: each row keeps a real `<form method="post">` that
//! posts to the existing delete route. The script intercepts the submit and
//! opens a themed `<dialog class="cpn-modal">`; Confirm submits the original
//! form, Cancel closes the dialog. Without `<dialog>` support the script falls
//! back to `window.confirm` naming the passkey label.

/// Escape text for HTML text and attribute contexts.
fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Per-row Remove form. Posts to the existing delete route; the page script
/// shows the confirmation before the request is sent.
pub fn passkey_remove_form_html(id: &str, label: &str) -> String {
    format!(
        r#"<form method="post" action="/account/users/profile/passkey/delete" style="display:inline;" data-passkey-remove-form data-passkey-label="{label}">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="linkish" style="background:none;border:0;color:#d92d20;font-weight:600;cursor:pointer;padding:0;" aria-label="Remove passkey {label}">Remove</button>
                </form>"#,
        id = html_escape(id),
        label = html_escape(label),
    )
}

/// Shared confirmation dialog plus its script. Emit once per page.
pub fn passkey_remove_modal_html() -> &'static str {
    r#"<dialog id="cpn-passkey-remove-modal" class="cpn-modal" aria-labelledby="cpn-passkey-remove-title" aria-describedby="cpn-passkey-remove-desc">
  <div class="stack-form" style="padding:20px;margin:0;">
    <h2 id="cpn-passkey-remove-title">Remove passkey?</h2>
    <p id="cpn-passkey-remove-desc" class="muted" style="margin:0 0 4px;">Are you sure you want to remove the passkey <strong id="cpn-passkey-remove-name"></strong>? You will no longer be able to sign in with it. This cannot be undone.</p>
    <div style="display:flex;gap:8px;justify-content:flex-end;margin-top:16px;">
      <button type="button" class="btn-secondary" id="cpn-passkey-remove-cancel">Cancel</button>
      <button type="button" class="btn-primary" id="cpn-passkey-remove-confirm" style="background:#d92d20;border-color:#d92d20;">Remove</button>
    </div>
  </div>
</dialog>
<script>
(function () {
  if (window.__cpnPasskeyRemoveBound) return;
  window.__cpnPasskeyRemoveBound = true;
  var pending = null;
  function modal() { return document.getElementById("cpn-passkey-remove-modal"); }
  function nameOf(form) {
    var label = form.getAttribute("data-passkey-label") || "";
    return label.trim() || "this passkey";
  }
  document.addEventListener("submit", function (ev) {
    var form = ev.target;
    if (!form || !form.hasAttribute || !form.hasAttribute("data-passkey-remove-form")) return;
    if (form.getAttribute("data-passkey-confirmed") === "1") return;
    ev.preventDefault();
    var dlg = modal();
    var name = nameOf(form);
    if (dlg && typeof dlg.showModal === "function") {
      pending = form;
      var target = document.getElementById("cpn-passkey-remove-name");
      if (target) target.textContent = name;
      dlg.showModal();
      var cancel = document.getElementById("cpn-passkey-remove-cancel");
      if (cancel) cancel.focus();
      return;
    }
    if (window.confirm("Remove passkey " + name + "? You will no longer be able to sign in with it.")) {
      form.setAttribute("data-passkey-confirmed", "1");
      form.submit();
    }
  }, true);
  document.addEventListener("click", function (ev) {
    var t = ev.target;
    if (!t || !t.id) return;
    var dlg = modal();
    if (t.id === "cpn-passkey-remove-cancel") {
      pending = null;
      if (dlg && dlg.close) dlg.close();
    } else if (t.id === "cpn-passkey-remove-confirm") {
      var form = pending;
      pending = null;
      if (dlg && dlg.close) dlg.close();
      if (form) {
        form.setAttribute("data-passkey-confirmed", "1");
        form.submit();
      }
    }
  });
  document.addEventListener("close", function (ev) {
    if (ev.target && ev.target.id === "cpn-passkey-remove-modal") pending = null;
  }, true);
})();
</script>"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_form_posts_to_existing_route_and_escapes_label() {
        let html = passkey_remove_form_html("abc123", "Laptop \"<b>\"");
        assert!(html.contains(r#"action="/account/users/profile/passkey/delete""#));
        assert!(html.contains(r#"name="id" value="abc123""#));
        assert!(html.contains("data-passkey-remove-form"));
        assert!(!html.contains("<b>"));
        assert!(html.contains("&lt;b&gt;"));
        assert!(!html.contains("onsubmit=\"return confirm("));
    }

    #[test]
    fn modal_has_accessible_cancel_and_remove_buttons() {
        let html = passkey_remove_modal_html();
        assert!(html.contains(r#"class="cpn-modal""#));
        assert!(html.contains("aria-labelledby=\"cpn-passkey-remove-title\""));
        assert!(html.contains(">Cancel</button>"));
        assert!(html.contains(">Remove</button>"));
        assert!(html.contains("Are you sure"));
        assert!(html.contains("window.confirm"));
        assert!(!html.contains('\u{2014}'));
    }
}

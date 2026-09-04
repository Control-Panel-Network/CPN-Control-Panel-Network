//! Sidebar footer actions: notifications, account settings, icon-only theme toggle.

use crate::panel_notifications::{load_notifications, unread_count};
use crate::panel_theme::ColorMode;
use crate::panel_theme_chrome::sidebar_theme_toggle;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn bell_svg() -> &'static str {
    r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>"#
}

fn gear_svg() -> &'static str {
    r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>"#
}

pub fn sidebar_footer_styles() -> &'static str {
    r#"
.sidebar-footer {
  flex:0 0 auto; display:flex; align-items:center; gap:4px;
  padding-top:18px; border-top:1px solid var(--hairline);
  position:relative;
}
.sidebar-footer-actions {
  display:flex; align-items:center; gap:2px; min-width:0;
}
.footer-icon-btn {
  position:relative; width:40px; height:40px; flex:0 0 auto;
  display:inline-grid; place-items:center; border:0; border-radius:10px;
  background:transparent; color:var(--muted); padding:0; text-decoration:none;
}
.footer-icon-btn:hover { background:rgba(0,0,0,.06); color:var(--ink); }
.footer-icon-btn:focus-visible {
  outline:2px solid var(--blue-focus); outline-offset:2px;
}
.theme-toggle.footer-icon-btn {
  width:40px; min-height:40px; margin:0; padding:0; gap:0;
  border:0; border-radius:10px; background:transparent; font-size:16px;
  justify-content:center; text-align:center;
}
.theme-toggle.footer-icon-btn .theme-toggle-icon {
  width:auto; height:auto; border:0; border-radius:0; background:transparent;
  font-size:16px; line-height:1;
}
.sidebar-footer .logout {
  margin-left:auto; display:inline-flex; align-items:center; justify-content:center;
  min-height:40px; min-width:44px; padding:0 10px; color:var(--muted); font-size:13px;
  white-space:nowrap;
}
.sidebar-footer .logout:hover { color:var(--ink); }
.notify-wrap { position:relative; }
.notify-badge {
  position:absolute; top:4px; right:4px; min-width:16px; height:16px; padding:0 4px;
  border-radius:999px; background:#dc2626; color:#fff; font-size:10px; font-weight:700;
  line-height:16px; text-align:center; pointer-events:none;
}
.notify-badge[hidden] { display:none !important; }
/* Fixed + body portal (JS) so sticky sidebar overflow cannot clip the panel. */
.notify-popover {
  position:fixed; z-index:120;
  width:340px; max-width:calc(100vw - 24px); max-height:min(360px, 50vh);
  display:flex; flex-direction:column; gap:0;
  background:var(--canvas); color:var(--ink); border:1px solid var(--hairline);
  border-radius:14px; box-shadow:0 12px 32px rgba(0,0,0,.14); overflow:hidden;
}
.notify-popover[hidden] { display:none !important; }
.notify-popover header {
  display:flex; align-items:center; justify-content:space-between; gap:10px;
  padding:12px 14px; border-bottom:1px solid var(--hairline); font-size:14px; font-weight:600;
  flex:0 0 auto;
}
.notify-popover header span { flex:1 1 auto; min-width:0; }
.notify-popover header button {
  flex:0 0 auto; border:0; background:transparent; color:var(--blue); font:inherit;
  font-size:12px; font-weight:600; padding:4px 6px; cursor:pointer; white-space:nowrap;
}
.notify-list {
  list-style:none; margin:0; padding:0; overflow-y:auto; flex:1 1 auto; min-height:0;
}
.notify-list li {
  padding:10px 14px; border-bottom:1px solid var(--hairline); font-size:13px;
}
.notify-list li:last-child { border-bottom:0; }
.notify-list li.unread { background:rgba(0,102,204,.06); }
.notify-list strong { display:block; font-size:13px; margin-bottom:2px; }
.notify-list span { color:var(--muted); font-size:12px; line-height:1.4; }
.notify-empty {
  margin:0; padding:20px 14px; color:var(--muted); font-size:13px; text-align:center;
}
[data-color-mode="dark"] .footer-icon-btn:hover { background:rgba(255,255,255,.08); }
[data-color-mode="dark"] .notify-list li.unread { background:rgba(59,130,246,.12); }
[data-color-mode="dark"] .notify-popover {
  box-shadow:0 12px 32px rgba(0,0,0,.45);
}
"#
}

pub fn sidebar_footer_markup(username: &str, color_mode: ColorMode) -> String {
    let store = load_notifications(username);
    let unread = unread_count(&store);
    let badge = if unread > 0 {
        let label = if unread > 99 {
            "99+".to_string()
        } else {
            unread.to_string()
        };
        format!(
            r#"<span class="notify-badge" id="cpn-notify-badge">{}</span>"#,
            html_escape(&label)
        )
    } else {
        r#"<span class="notify-badge" id="cpn-notify-badge" hidden>0</span>"#.to_string()
    };
    let toggle = sidebar_theme_toggle(color_mode);
    format!(
        r#"<div class="sidebar-footer-actions">
          <div class="notify-wrap">
            <button type="button" id="cpn-notify-btn" class="footer-icon-btn"
              aria-expanded="false" aria-controls="cpn-notify-panel"
              aria-label="Notifications" title="Notifications">
              {bell}
              {badge}
            </button>
            <div id="cpn-notify-panel" class="notify-popover" hidden role="dialog" aria-label="Notifications">
              <header>
                <span>Notifications</span>
                <button type="button" id="cpn-notify-mark-all">Mark all as read</button>
              </header>
              <ul class="notify-list" id="cpn-notify-list"></ul>
              <p class="notify-empty" id="cpn-notify-empty">No notifications yet.</p>
            </div>
          </div>
          <a class="footer-icon-btn" href="/account/users/profile"
            aria-label="Account settings" title="Account settings">{gear}</a>
          {toggle}
        </div>
        <a class="logout" href="/logout">Log out</a>"#,
        bell = bell_svg(),
        badge = badge,
        gear = gear_svg(),
        toggle = toggle,
    )
}

pub fn notifications_popover_script() -> &'static str {
    r#"
<script>
(function () {
  var btn = document.getElementById("cpn-notify-btn");
  var panel = document.getElementById("cpn-notify-panel");
  var list = document.getElementById("cpn-notify-list");
  var empty = document.getElementById("cpn-notify-empty");
  var badge = document.getElementById("cpn-notify-badge");
  var markAll = document.getElementById("cpn-notify-mark-all");
  var wrap = btn ? btn.closest(".notify-wrap") : null;
  if (!btn || !panel || !list || !empty) return;

  var POPOVER_WIDTH = 340;
  var POPOVER_GAP = 8;
  var VIEWPORT_PAD = 12;

  function placePanel() {
    var rect = btn.getBoundingClientRect();
    var width = Math.min(POPOVER_WIDTH, window.innerWidth - VIEWPORT_PAD * 2);
    var left = rect.left;
    if (left + width > window.innerWidth - VIEWPORT_PAD) {
      left = Math.max(VIEWPORT_PAD, window.innerWidth - width - VIEWPORT_PAD);
    }
    if (left < VIEWPORT_PAD) left = VIEWPORT_PAD;
    var bottom = Math.max(VIEWPORT_PAD, window.innerHeight - rect.top + POPOVER_GAP);
    panel.style.left = left + "px";
    panel.style.bottom = bottom + "px";
    panel.style.width = width + "px";
    panel.style.right = "auto";
    panel.style.top = "auto";
    if (panel.parentElement !== document.body) {
      document.body.appendChild(panel);
    }
  }

  function restorePanel() {
    if (wrap && panel.parentElement !== wrap) {
      wrap.appendChild(panel);
    }
    panel.style.left = "";
    panel.style.bottom = "";
    panel.style.width = "";
    panel.style.right = "";
    panel.style.top = "";
  }

  function setOpen(open) {
    if (open) {
      placePanel();
      panel.hidden = false;
    } else {
      panel.hidden = true;
      restorePanel();
    }
    btn.setAttribute("aria-expanded", open ? "true" : "false");
  }

  function updateBadge(count) {
    if (!badge) return;
    if (count > 0) {
      badge.hidden = false;
      badge.textContent = count > 99 ? "99+" : String(count);
    } else {
      badge.hidden = true;
      badge.textContent = "0";
    }
  }

  function escapeHtml(value) {
    return String(value || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function renderItems(payload) {
    var items = (payload && payload.items) || [];
    updateBadge((payload && payload.unread_count) || 0);
    list.innerHTML = "";
    if (!items.length) {
      empty.hidden = false;
      return;
    }
    empty.hidden = true;
    items.forEach(function (item) {
      var li = document.createElement("li");
      if (!item.read) li.className = "unread";
      li.innerHTML = "<strong>" + escapeHtml(item.title) + "</strong>"
        + (item.body ? "<span>" + escapeHtml(item.body) + "</span>" : "");
      list.appendChild(li);
    });
  }

  function loadNotifications() {
    return fetch("/api/panel/notifications", {
      credentials: "same-origin",
      headers: { "Accept": "application/json" }
    }).then(function (res) {
      return res.json().then(function (data) {
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      });
    }).then(function (data) {
      renderItems(data);
      return data;
    }).catch(function () {
      empty.hidden = false;
      empty.textContent = "Could not load notifications.";
      list.innerHTML = "";
    });
  }

  btn.addEventListener("click", function (ev) {
    ev.stopPropagation();
    var next = panel.hidden;
    setOpen(next);
    if (next) loadNotifications();
  });

  document.addEventListener("click", function (ev) {
    if (panel.hidden) return;
    var target = ev.target;
    if (panel.contains(target) || btn.contains(target)) return;
    setOpen(false);
  });

  document.addEventListener("keydown", function (ev) {
    if (ev.key === "Escape" && !panel.hidden) setOpen(false);
  });

  window.addEventListener("resize", function () {
    if (!panel.hidden) placePanel();
  });
  window.addEventListener("scroll", function () {
    if (!panel.hidden) placePanel();
  }, true);

  if (markAll) {
    markAll.addEventListener("click", function (ev) {
      ev.stopPropagation();
      fetch("/api/panel/notifications/mark-read", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", "Accept": "application/json" },
        body: JSON.stringify({ all: true })
      }).then(function (res) { return res.json(); })
        .then(function (data) { if (data && data.ok) renderItems(data); })
        .catch(function () {});
    });
  }
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_notifications::push_notification;

    #[test]
    fn footer_includes_actions_and_logout() {
        with_test_data_dir(|| {
            let _ = push_notification("Admin", "Plugin updated", "MAS plugin message", "plugin");
            let html = sidebar_footer_markup("Admin", ColorMode::Light);
            assert!(html.contains("cpn-notify-btn"));
            assert!(html.contains("/account/users/profile"));
            assert!(html.contains("cpn-color-toggle"));
            assert!(html.contains(">Log out</a>"));
            assert!(html.contains("Mark all as read"));
            assert!(!html.contains("Light mode"));
            assert!(!html.contains("Dark mode"));
            assert!(html.contains("notify-badge"));
            assert!(!html.contains("CyberPanel"));
        });
    }

    #[test]
    fn notify_styles_use_fixed_popover() {
        let css = sidebar_footer_styles();
        assert!(css.contains("position:fixed"));
        assert!(css.contains("z-index:120"));
        assert!(css.contains("white-space:nowrap"));
        assert!(css.contains("width:340px"));
    }

    #[test]
    fn notify_script_portals_to_body() {
        let js = notifications_popover_script();
        assert!(js.contains("document.body.appendChild(panel)"));
        assert!(js.contains("placePanel"));
        assert!(js.contains("POPOVER_WIDTH = 340"));
    }
}

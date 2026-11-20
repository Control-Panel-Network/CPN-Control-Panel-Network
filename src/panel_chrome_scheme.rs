//! Browser-local accent schemes for panel chrome (CyberPanel-style "Make it yours").

pub const CHROME_SCHEME_STORAGE_KEY: &str = "cpn-chrome-scheme";
pub const DEFAULT_CHROME_SCHEME: &str = "evergreen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromeSchemeEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub default_note: bool,
    pub swatch: &'static str,
    pub accent_light: &'static str,
    pub focus_light: &'static str,
    pub accent_dark: &'static str,
    pub focus_dark: &'static str,
    pub green_light: &'static str,
    pub green_dark: &'static str,
}

pub const CHROME_SCHEMES: &[ChromeSchemeEntry] = &[
    ChromeSchemeEntry {
        id: "evergreen",
        label: "Evergreen",
        default_note: true,
        swatch: "linear-gradient(135deg,#1f7a4c 0%,#14532d 100%)",
        accent_light: "#18864b",
        focus_light: "#1f9d5c",
        accent_dark: "#3dd68c",
        focus_dark: "#6ee7a8",
        green_light: "#18864b",
        green_dark: "#3dd68c",
    },
    ChromeSchemeEntry {
        id: "ocean-blue",
        label: "Ocean Blue",
        default_note: false,
        swatch: "linear-gradient(135deg,#0b7fd4 0%,#0059a0 100%)",
        accent_light: "#0077be",
        focus_light: "#0088d9",
        accent_dark: "#38bdf8",
        focus_dark: "#7dd3fc",
        green_light: "#18864b",
        green_dark: "#3dd68c",
    },
    ChromeSchemeEntry {
        id: "cloud-teal",
        label: "Cloud Teal",
        default_note: false,
        swatch: "linear-gradient(135deg,#2dd4bf 0%,#0f766e 100%)",
        accent_light: "#0d9488",
        focus_light: "#14b8a6",
        accent_dark: "#2dd4bf",
        focus_dark: "#5eead4",
        green_light: "#0d9488",
        green_dark: "#2dd4bf",
    },
    ChromeSchemeEntry {
        id: "violet",
        label: "Violet",
        default_note: false,
        swatch: "linear-gradient(135deg,#8b5cf6 0%,#5b21b6 100%)",
        accent_light: "#6d28d9",
        focus_light: "#8b5cf6",
        accent_dark: "#a78bfa",
        focus_dark: "#c4b5fd",
        green_light: "#18864b",
        green_dark: "#3dd68c",
    },
    ChromeSchemeEntry {
        id: "slate",
        label: "Slate",
        default_note: false,
        swatch: "linear-gradient(135deg,#64748b 0%,#334155 100%)",
        accent_light: "#475569",
        focus_light: "#64748b",
        accent_dark: "#94a3b8",
        focus_dark: "#cbd5e1",
        green_light: "#18864b",
        green_dark: "#3dd68c",
    },
    ChromeSchemeEntry {
        id: "bright",
        label: "Bright",
        default_note: false,
        swatch: "linear-gradient(135deg,#3b82f6 0%,#1d4ed8 100%)",
        accent_light: "#2563eb",
        focus_light: "#3b82f6",
        accent_dark: "#60a5fa",
        focus_dark: "#93c5fd",
        green_light: "#18864b",
        green_dark: "#3dd68c",
    },
    ChromeSchemeEntry {
        id: "apple",
        label: "Apple",
        default_note: false,
        swatch: "linear-gradient(135deg,#409cff 0%,#0066cc 100%)",
        accent_light: "#0066cc",
        focus_light: "#007aff",
        accent_dark: "#409cff",
        focus_dark: "#7cb8ff",
        green_light: "#18864b",
        green_dark: "#3dd68c",
    },
];

pub fn parse_chrome_scheme(raw: &str) -> Option<&'static str> {
    let id = raw.trim().to_ascii_lowercase();
    CHROME_SCHEMES
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.id)
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn palette_svg() -> &'static str {
    r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="13.5" cy="6.5" r="2.5"/><circle cx="17.5" cy="10.5" r="2.5"/><circle cx="8.5" cy="7.5" r="2.5"/><circle cx="6.5" cy="12.5" r="2.5"/><path d="M12 22a10 10 0 0 0 10-10c0-2.2-1.7-4-3.8-4.4L16 4.6A10 10 0 0 0 2 12a10 10 0 0 0 10 10z"/></svg>"#
}

fn scheme_vars_block(entry: &ChromeSchemeEntry) -> String {
    format!(
        r#"html[data-chrome-scheme="{id}"][data-color-mode="light"],
html[data-chrome-scheme="{id}"]:not([data-color-mode="dark"]) {{
  --cpn-accent:{al}; --cpn-accent-focus:{fl}; --blue:{al}; --blue-focus:{fl}; --green:{gl};
}}
html[data-chrome-scheme="{id}"][data-color-mode="dark"] {{
  --cpn-accent:{ad}; --cpn-accent-focus:{fd}; --blue:{ad}; --blue-focus:{fd}; --green:{gd};
}}
"#,
        id = entry.id,
        al = entry.accent_light,
        fl = entry.focus_light,
        ad = entry.accent_dark,
        fd = entry.focus_dark,
        gl = entry.green_light,
        gd = entry.green_dark,
    )
}

pub fn chrome_scheme_styles() -> String {
    let mut css = String::from(
        r#"
.scheme-wrap { position:relative; }
.chrome-scheme-popover {
  position:fixed; z-index:121;
  width:min(320px, calc(100vw - 24px)); max-height:min(420px, 70vh);
  display:flex; flex-direction:column; gap:0;
  background:var(--canvas); color:var(--ink); border:1px solid var(--hairline);
  border-radius:14px; box-shadow:0 12px 32px rgba(0,0,0,.16); overflow:hidden;
}
.chrome-scheme-popover[hidden] { display:none !important; }
.chrome-scheme-popover header {
  padding:14px 16px 12px; border-bottom:1px solid var(--hairline);
}
.chrome-scheme-popover header strong {
  display:block; font-size:15px; font-weight:700; margin:0 0 4px;
}
.chrome-scheme-popover header p {
  margin:0; font-size:12px; line-height:1.4; color:var(--muted);
}
.chrome-scheme-list {
  list-style:none; margin:0; padding:8px; overflow-y:auto;
  display:flex; flex-direction:column; gap:2px;
}
.chrome-scheme-option {
  width:100%; display:flex; align-items:center; gap:12px;
  padding:10px 10px; border:0; border-radius:10px;
  background:transparent; color:var(--ink); font:inherit; font-size:14px;
  text-align:left; cursor:pointer;
}
.chrome-scheme-option:hover,
.chrome-scheme-option:focus-visible {
  background:rgba(0,0,0,.06); outline:none;
}
.chrome-scheme-option.is-active {
  background:color-mix(in srgb, var(--blue) 12%, transparent);
}
.chrome-scheme-swatch {
  width:28px; height:28px; flex:0 0 auto; border-radius:6px;
  box-shadow:inset 0 0 0 1px rgba(0,0,0,.12);
}
.chrome-scheme-label { flex:1 1 auto; min-width:0; }
.chrome-scheme-label small {
  font-size:12px; color:var(--muted); font-weight:400;
}
.chrome-scheme-check {
  flex:0 0 auto; width:18px; height:18px; color:var(--blue);
  opacity:0;
}
.chrome-scheme-option.is-active .chrome-scheme-check { opacity:1; }
[data-color-mode="dark"] .chrome-scheme-popover {
  background:#12151c; color:#e8edf5; border-color:rgba(255,255,255,.1);
  box-shadow:0 12px 32px rgba(0,0,0,.45);
}
[data-color-mode="dark"] .chrome-scheme-popover header {
  border-bottom-color:rgba(255,255,255,.1);
}
[data-color-mode="dark"] .chrome-scheme-option:hover,
[data-color-mode="dark"] .chrome-scheme-option:focus-visible {
  background:rgba(255,255,255,.08);
}
"#,
    );
    for entry in CHROME_SCHEMES {
        css.push_str(&scheme_vars_block(entry));
    }
    css
}

pub fn chrome_scheme_boot_script() -> &'static str {
    r#"<script>
(function () {
  var KEY = "cpn-chrome-scheme";
  var FALLBACK = "evergreen";
  var allowed = {evergreen:1,"ocean-blue":1,"cloud-teal":1,violet:1,slate:1,bright:1,apple:1};
  function apply(id) {
    if (!allowed[id]) id = FALLBACK;
    document.documentElement.setAttribute("data-chrome-scheme", id);
    return id;
  }
  try {
    var stored = window.localStorage.getItem(KEY);
    apply(stored && allowed[stored] ? stored : FALLBACK);
  } catch (e) {
    apply(FALLBACK);
  }
})();
</script>"#
}

pub fn sidebar_chrome_scheme_markup() -> String {
    let mut items = String::new();
    for entry in CHROME_SCHEMES {
        let label = if entry.default_note {
            format!(
                r#"{main} <small>(default)</small>"#,
                main = html_escape(entry.label)
            )
        } else {
            html_escape(entry.label)
        };
        items.push_str(&format!(
            r#"<li><button type="button" class="chrome-scheme-option" data-scheme="{id}" aria-pressed="false">
  <span class="chrome-scheme-swatch" style="background:{swatch}" aria-hidden="true"></span>
  <span class="chrome-scheme-label">{label}</span>
  <svg class="chrome-scheme-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 13l4 4L19 7"/></svg>
</button></li>"#,
            id = entry.id,
            swatch = entry.swatch,
            label = label,
        ));
    }
    format!(
        r#"<div class="scheme-wrap">
  <button type="button" id="cpn-chrome-scheme-btn" class="footer-icon-btn"
    aria-expanded="false" aria-controls="cpn-chrome-scheme-popover"
    aria-label="Color scheme" title="Color scheme">
    {palette}
  </button>
  <div id="cpn-chrome-scheme-popover" class="chrome-scheme-popover" hidden role="dialog" aria-label="Make it yours">
    <header>
      <strong>Make it yours</strong>
      <p>Choose a color scheme. Saved in this browser.</p>
    </header>
    <ul class="chrome-scheme-list" id="cpn-chrome-scheme-list">{items}</ul>
  </div>
</div>"#,
        palette = palette_svg(),
        items = items,
    )
}

pub fn chrome_scheme_popover_script() -> &'static str {
    r#"
<script>
(function () {
  var KEY = "cpn-chrome-scheme";
  var FALLBACK = "evergreen";
  var allowed = {evergreen:1,"ocean-blue":1,"cloud-teal":1,violet:1,slate:1,bright:1,apple:1};
  var btn = document.getElementById("cpn-chrome-scheme-btn");
  var panel = document.getElementById("cpn-chrome-scheme-popover");
  var list = document.getElementById("cpn-chrome-scheme-list");
  var wrap = btn ? btn.closest(".scheme-wrap") : null;
  if (!btn || !panel || !list) return;

  var POPOVER_WIDTH = 320;
  var POPOVER_GAP = 10;
  var VIEWPORT_PAD = 12;

  function currentScheme() {
    var attr = document.documentElement.getAttribute("data-chrome-scheme") || FALLBACK;
    return allowed[attr] ? attr : FALLBACK;
  }

  function applyScheme(id, persist) {
    if (!allowed[id]) id = FALLBACK;
    document.documentElement.setAttribute("data-chrome-scheme", id);
    if (persist !== false) {
      try { window.localStorage.setItem(KEY, id); } catch (e) {}
    }
    list.querySelectorAll(".chrome-scheme-option").forEach(function (el) {
      var active = el.getAttribute("data-scheme") === id;
      el.classList.toggle("is-active", active);
      el.setAttribute("aria-pressed", active ? "true" : "false");
    });
  }

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
      applyScheme(currentScheme(), false);
      placePanel();
      panel.hidden = false;
    } else {
      panel.hidden = true;
      restorePanel();
    }
    btn.setAttribute("aria-expanded", open ? "true" : "false");
  }

  applyScheme(currentScheme(), false);

  btn.addEventListener("click", function (ev) {
    ev.stopPropagation();
    setOpen(panel.hidden);
  });

  list.addEventListener("click", function (ev) {
    var target = ev.target;
    while (target && target !== list) {
      if (target.classList && target.classList.contains("chrome-scheme-option")) {
        var id = target.getAttribute("data-scheme");
        if (id) applyScheme(id, true);
        break;
      }
      target = target.parentElement;
    }
  });

  panel.addEventListener("mousedown", function (ev) {
    ev.stopPropagation();
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
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_schemes() {
        assert_eq!(parse_chrome_scheme("ocean-blue"), Some("ocean-blue"));
        assert_eq!(parse_chrome_scheme(" Apple "), Some("apple"));
        assert_eq!(parse_chrome_scheme("nope"), None);
    }

    #[test]
    fn markup_includes_popover_and_schemes() {
        let html = sidebar_chrome_scheme_markup();
        assert!(html.contains("Make it yours"));
        assert!(html.contains("Saved in this browser"));
        assert!(html.contains("cpn-chrome-scheme-btn"));
        assert!(html.contains(r#"data-scheme="slate""#));
        assert!(html.contains("Evergreen"));
        assert!(html.contains("(default)"));
    }

    #[test]
    fn styles_emit_scheme_variables() {
        let css = chrome_scheme_styles();
        assert!(css.contains(r#"html[data-chrome-scheme="evergreen"]"#));
        assert!(css.contains(r#"html[data-chrome-scheme="ocean-blue"]"#));
        assert!(css.contains("--blue:#0077be"));
        assert!(css.contains("chrome-scheme-popover"));
    }

    #[test]
    fn boot_script_uses_storage_key() {
        let js = chrome_scheme_boot_script();
        assert!(js.contains(CHROME_SCHEME_STORAGE_KEY));
        assert!(js.contains("data-chrome-scheme"));
    }
}
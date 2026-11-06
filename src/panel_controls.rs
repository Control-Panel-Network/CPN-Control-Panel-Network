//! Shared checkbox (Option C) and switch (Option D) styles for the CPN Panel.
//!
//! - **C Soft rounded square + focus ring:** default `input[type=checkbox]`
//!   (secondary flags, one-shot actions, multi-select).
//! - **D iOS-like switch:** `input.cpn-switch` with `role="switch"` for
//!   persistent Enable/Disable master toggles.

/// Class + ARIA attributes for Option D master toggles.
pub const SWITCH_ATTRS: &str = r#"class="cpn-switch" role="switch""#;

/// Shared CSS tokens and control chrome (panel shell + auth pages).
pub fn control_styles() -> &'static str {
    r#"
/* CPN controls: Option C (checkbox) + Option D (switch) */
:root {
  --cpn-control-border:#64748b;
  --cpn-control-bg:color-mix(in srgb, var(--canvas, #fff) 86%, #0f172a);
  --cpn-control-on:var(--cpn-accent, var(--blue, #0066cc));
  --cpn-control-on-ink:#fff;
  --cpn-control-focus:var(--blue-focus, var(--cpn-accent-focus, #5eead4));
  --cpn-switch-off:#cbd5e1;
  --cpn-switch-knob:#f8fafc;
  --cpn-switch-knob-on:#fff;
}
[data-color-mode="dark"] {
  --cpn-control-border:#64748b;
  --cpn-control-bg:color-mix(in srgb, var(--canvas, #1a1d26) 80%, #000);
  --cpn-switch-off:#334155;
  --cpn-switch-knob:#e2e8f0;
  --cpn-switch-knob-on:#0f172a;
}
input[type=checkbox]:not(.cpn-switch) {
  -webkit-appearance:none;
  appearance:none;
  width:18px;
  height:18px;
  min-width:18px;
  min-height:18px;
  margin:0;
  padding:0;
  flex:0 0 auto;
  box-sizing:border-box;
  border:1.5px solid var(--cpn-control-border);
  border-radius:5px;
  background-color:var(--cpn-control-bg);
  background-repeat:no-repeat;
  background-position:center;
  background-size:11px 11px;
  color:transparent;
  cursor:pointer;
  vertical-align:middle;
  transition:background-color .15s ease, border-color .15s ease, box-shadow .15s ease;
}
input[type=checkbox]:not(.cpn-switch):hover:not(:disabled):not(:checked):not(:indeterminate) {
  border-color:var(--cpn-control-on);
  background-color:color-mix(in srgb, var(--cpn-control-on) 14%, var(--canvas, #1a1d26));
}
input[type=checkbox]:not(.cpn-switch):checked,
input[type=checkbox]:not(.cpn-switch):indeterminate {
  background-color:var(--cpn-control-on);
  border-color:var(--cpn-control-on);
  color:var(--cpn-control-on-ink);
}
input[type=checkbox]:not(.cpn-switch):checked {
  background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12' fill='none'%3E%3Cpath d='M2.2 6.2 L4.8 8.8 L9.8 3.2' stroke='%230f172a' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
}
[data-color-mode="light"] input[type=checkbox]:not(.cpn-switch):checked,
:root:not([data-color-mode="dark"]) input[type=checkbox]:not(.cpn-switch):checked {
  background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12' fill='none'%3E%3Cpath d='M2.2 6.2 L4.8 8.8 L9.8 3.2' stroke='%23ffffff' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
}
[data-color-mode="dark"] input[type=checkbox]:not(.cpn-switch):checked {
  background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12' fill='none'%3E%3Cpath d='M2.2 6.2 L4.8 8.8 L9.8 3.2' stroke='%230f172a' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
}
input[type=checkbox]:not(.cpn-switch):indeterminate {
  background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12' fill='none'%3E%3Cpath d='M2.5 6 H9.5' stroke='%230f172a' stroke-width='2.4' stroke-linecap='round'/%3E%3C/svg%3E");
}
[data-color-mode="light"] input[type=checkbox]:not(.cpn-switch):indeterminate,
:root:not([data-color-mode="dark"]) input[type=checkbox]:not(.cpn-switch):indeterminate {
  background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12' fill='none'%3E%3Cpath d='M2.5 6 H9.5' stroke='%23ffffff' stroke-width='2.4' stroke-linecap='round'/%3E%3C/svg%3E");
}
input[type=checkbox]:not(.cpn-switch):focus-visible {
  outline:2px solid var(--cpn-control-focus);
  outline-offset:2px;
  border-color:var(--cpn-control-focus);
}
input[type=checkbox]:not(.cpn-switch):disabled {
  opacity:.45;
  cursor:not-allowed;
}
input[type=checkbox].cpn-switch {
  -webkit-appearance:none;
  appearance:none;
  width:42px;
  height:24px;
  min-width:42px;
  min-height:24px;
  margin:0;
  padding:0;
  flex:0 0 auto;
  box-sizing:border-box;
  border:1px solid var(--hairline, #334155);
  border-radius:999px;
  background:var(--cpn-switch-off);
  position:relative;
  cursor:pointer;
  vertical-align:middle;
  transition:background-color .15s ease, border-color .15s ease;
}
input[type=checkbox].cpn-switch::after {
  content:"";
  position:absolute;
  top:2px;
  left:2px;
  width:18px;
  height:18px;
  border-radius:50%;
  background:var(--cpn-switch-knob);
  box-shadow:0 1px 2px rgba(15,23,42,.22);
  transition:left .15s ease, background-color .15s ease;
}
input[type=checkbox].cpn-switch:hover:not(:disabled):not(:checked) {
  background:#3f5169;
}
[data-color-mode="light"] input[type=checkbox].cpn-switch:hover:not(:disabled):not(:checked),
:root:not([data-color-mode="dark"]) input[type=checkbox].cpn-switch:hover:not(:disabled):not(:checked) {
  background:#94a3b8;
}
input[type=checkbox].cpn-switch:checked {
  background:var(--cpn-control-on);
  border-color:var(--cpn-control-on);
}
input[type=checkbox].cpn-switch:checked::after {
  left:20px;
  background:var(--cpn-switch-knob-on);
}
input[type=checkbox].cpn-switch:focus-visible {
  outline:2px solid var(--cpn-control-focus);
  outline-offset:2px;
}
input[type=checkbox].cpn-switch:disabled {
  opacity:.45;
  cursor:not-allowed;
}
label:has(> input[type=checkbox]),
label.cpn-check-item,
.cpn-check-item,
.remember,
.cpn-minimalist-toggle,
.docker-inline-check {
  cursor:pointer;
}
@media (prefers-reduced-motion:reduce) {
  input[type=checkbox]:not(.cpn-switch),
  input[type=checkbox].cpn-switch,
  input[type=checkbox].cpn-switch::after {
    transition:none;
  }
}
"#
}

#[cfg(test)]
mod tests {
    use super::{SWITCH_ATTRS, control_styles};

    #[test]
    fn control_styles_cover_option_c_and_d() {
        let css = control_styles();
        assert!(css.contains("input[type=checkbox]:not(.cpn-switch)"));
        assert!(css.contains("border-radius:5px"));
        assert!(css.contains("input[type=checkbox].cpn-switch"));
        assert!(css.contains("border-radius:999px"));
        assert!(css.contains(":focus-visible"));
        assert!(css.contains("prefers-reduced-motion"));
        assert!(css.contains("--cpn-control-on"));
    }

    #[test]
    fn switch_attrs_are_accessible() {
        assert!(SWITCH_ATTRS.contains("cpn-switch"));
        assert!(SWITCH_ATTRS.contains(r#"role="switch""#));
    }
}

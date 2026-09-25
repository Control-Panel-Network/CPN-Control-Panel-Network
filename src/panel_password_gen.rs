//! Client-side strong-password generator UI for Modify User / Create User forms.
//!
//! Preview fills the form password field so operators can copy and re-roll before submit.
//! Server policy and the blocked-password list still validate on submit.

use crate::account::{default_password_policy, MAX_PASSWORD_CHARS};

/// True when the password field is empty: server should generate (leave-blank UX).
/// When the field is filled (client preview), prefer that value even if Generate is checked.
pub fn wants_server_generated_password(password: &str) -> bool {
    password.trim().is_empty()
}

/// Checkbox, options panel, and regenerate controls for one password form.
pub fn password_gen_controls_html() -> String {
    let policy = default_password_policy();
    let min_len = policy.min_length.max(8);
    let max_len = MAX_PASSWORD_CHARS as u32;
    let default_len = min_len.max(20).min(max_len);
    let req_upper = if policy.require_uppercase { "1" } else { "0" };
    let req_number = if policy.require_number { "1" } else { "0" };
    let req_special = if policy.require_special { "1" } else { "0" };
    format!(
        r#"
        <div class="cpn-pw-gen" data-min="{min_len}" data-max="{max_len}" data-default-len="{default_len}"
             data-require-upper="{req_upper}" data-require-number="{req_number}" data-require-special="{req_special}">
          <label class="cpn-pw-gen-enable-label" style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
            <input name="generate" type="checkbox" value="1" class="cpn-pw-gen-enable">
            Generate a strong password
          </label>
          <div class="cpn-pw-gen-panel" hidden>
            <div class="cpn-pw-gen-row">
              <label class="cpn-pw-gen-length-label">Length
                <span class="cpn-pw-gen-length-value">{default_len}</span>
              </label>
              <div class="cpn-pw-gen-length-controls">
                <input type="range" class="cpn-pw-gen-length-range" min="{min_len}" max="{max_len}" value="{default_len}" aria-label="Password length">
                <input type="number" class="cpn-pw-gen-length-num" min="{min_len}" max="{max_len}" value="{default_len}" aria-label="Password length number">
              </div>
            </div>
            <div class="cpn-pw-gen-classes" role="group" aria-label="Character classes">
              <label class="cpn-pw-gen-class"><input type="checkbox" class="cpn-pw-gen-upper" checked> Uppercase</label>
              <label class="cpn-pw-gen-class"><input type="checkbox" class="cpn-pw-gen-lower" checked> Lowercase</label>
              <label class="cpn-pw-gen-class"><input type="checkbox" class="cpn-pw-gen-digit" checked> Numbers</label>
              <label class="cpn-pw-gen-class"><input type="checkbox" class="cpn-pw-gen-special"> Special</label>
            </div>
            <div class="cpn-pw-gen-actions">
              <button type="button" class="btn-secondary cpn-pw-gen-stronger">Stronger</button>
              <button type="button" class="btn-secondary cpn-pw-gen-regen">Regenerate</button>
              <button type="button" class="btn-secondary cpn-pw-gen-copy">Copy</button>
            </div>
            <p class="muted cpn-pw-gen-hint" style="margin:0;">Preview fills the password field above. Re-roll until you like it, then update. Server policy still applies on save.</p>
            <p class="muted cpn-pw-gen-status" role="status" style="margin:0;"></p>
          </div>
        </div>"#,
        min_len = min_len,
        max_len = max_len,
        default_len = default_len,
        req_upper = req_upper,
        req_number = req_number,
        req_special = req_special,
    )
}

/// Inline script (include once per page that renders [`password_gen_controls_html`]).
pub fn password_gen_script() -> String {
    r#"
<script>
(function () {
  if (window.cpnPwGenInit) return;
  window.cpnPwGenInit = true;

  var UPPER = "ABCDEFGHJKLMNPQRSTUVWXYZ";
  var LOWER = "abcdefghijkmnopqrstuvwxyz";
  var DIGIT = "23456789";
  var SPECIAL = "!@#$%&*+=?~-_.";

  function randIndex(max) {
    if (max <= 0) return 0;
    var buf = new Uint32Array(1);
    var limit = Math.floor(0x100000000 / max) * max;
    var x;
    do {
      crypto.getRandomValues(buf);
      x = buf[0];
    } while (x >= limit);
    return x % max;
  }

  function pick(pool) {
    return pool.charAt(randIndex(pool.length));
  }

  function shuffle(arr) {
    for (var i = arr.length - 1; i > 0; i--) {
      var j = randIndex(i + 1);
      var t = arr[i];
      arr[i] = arr[j];
      arr[j] = t;
    }
    return arr;
  }

  function clampLen(n, min, max) {
    n = parseInt(n, 10);
    if (isNaN(n)) n = min;
    return Math.max(min, Math.min(max, n));
  }

  function findPasswordInput(root) {
    var form = root.closest("form");
    if (!form) return null;
    return form.querySelector('input[name="password"]');
  }

  function setStatus(root, msg) {
    var el = root.querySelector(".cpn-pw-gen-status");
    if (el) el.textContent = msg || "";
  }

  function syncLengthUi(root, len) {
    var range = root.querySelector(".cpn-pw-gen-length-range");
    var num = root.querySelector(".cpn-pw-gen-length-num");
    var label = root.querySelector(".cpn-pw-gen-length-value");
    if (range) range.value = String(len);
    if (num) num.value = String(len);
    if (label) label.textContent = String(len);
  }

  function applyPolicyLocks(root) {
    var reqU = root.getAttribute("data-require-upper") === "1";
    var reqN = root.getAttribute("data-require-number") === "1";
    var reqS = root.getAttribute("data-require-special") === "1";
    var upper = root.querySelector(".cpn-pw-gen-upper");
    var digit = root.querySelector(".cpn-pw-gen-digit");
    var special = root.querySelector(".cpn-pw-gen-special");
    if (reqU && upper) { upper.checked = true; upper.disabled = true; }
    if (reqN && digit) { digit.checked = true; digit.disabled = true; }
    if (reqS && special) { special.checked = true; special.disabled = true; }
  }

  function buildPools(root) {
    var pools = [];
    var upper = root.querySelector(".cpn-pw-gen-upper");
    var lower = root.querySelector(".cpn-pw-gen-lower");
    var digit = root.querySelector(".cpn-pw-gen-digit");
    var special = root.querySelector(".cpn-pw-gen-special");
    if (upper && upper.checked) pools.push(UPPER);
    if (lower && lower.checked) pools.push(LOWER);
    if (digit && digit.checked) pools.push(DIGIT);
    if (special && special.checked) pools.push(SPECIAL);
    if (!pools.length) {
      pools.push(LOWER);
      if (lower) lower.checked = true;
    }
    return pools;
  }

  function generate(root) {
    var min = parseInt(root.getAttribute("data-min") || "8", 10);
    var max = parseInt(root.getAttribute("data-max") || "256", 10);
    var range = root.querySelector(".cpn-pw-gen-length-range");
    var len = clampLen(range ? range.value : root.getAttribute("data-default-len"), min, max);
    syncLengthUi(root, len);
    var pools = buildPools(root);
    var chars = [];
    for (var p = 0; p < pools.length; p++) {
      chars.push(pick(pools[p]));
    }
    var all = pools.join("");
    while (chars.length < len) {
      chars.push(pick(all));
    }
    shuffle(chars);
    return chars.join("").slice(0, len);
  }

  function fill(root, password) {
    var input = findPasswordInput(root);
    if (!input) return;
    input.type = "text";
    input.value = password;
    input.setAttribute("autocomplete", "new-password");
    setStatus(root, "Generated. Copy or regenerate before saving.");
  }

  function stronger(root) {
    var min = parseInt(root.getAttribute("data-min") || "8", 10);
    var max = parseInt(root.getAttribute("data-max") || "256", 10);
    var len = clampLen(Math.max(24, min), min, max);
    syncLengthUi(root, len);
    var upper = root.querySelector(".cpn-pw-gen-upper");
    var lower = root.querySelector(".cpn-pw-gen-lower");
    var digit = root.querySelector(".cpn-pw-gen-digit");
    var special = root.querySelector(".cpn-pw-gen-special");
    if (upper && !upper.disabled) upper.checked = true;
    if (lower) lower.checked = true;
    if (digit && !digit.disabled) digit.checked = true;
    if (special && !special.disabled) special.checked = true;
    fill(root, generate(root));
  }

  function setEnabled(root, on) {
    var panel = root.querySelector(".cpn-pw-gen-panel");
    var input = findPasswordInput(root);
    if (panel) panel.hidden = !on;
    if (!on) {
      if (input) {
        input.type = "password";
      }
      setStatus(root, "");
      return;
    }
    applyPolicyLocks(root);
    fill(root, generate(root));
  }

  function bind(root) {
    if (root.getAttribute("data-bound") === "1") return;
    root.setAttribute("data-bound", "1");
    applyPolicyLocks(root);
    var enable = root.querySelector(".cpn-pw-gen-enable");
    var range = root.querySelector(".cpn-pw-gen-length-range");
    var num = root.querySelector(".cpn-pw-gen-length-num");
    var regen = root.querySelector(".cpn-pw-gen-regen");
    var copyBtn = root.querySelector(".cpn-pw-gen-copy");
    var strongerBtn = root.querySelector(".cpn-pw-gen-stronger");
    var min = parseInt(root.getAttribute("data-min") || "8", 10);
    var max = parseInt(root.getAttribute("data-max") || "256", 10);

    if (enable) {
      enable.addEventListener("change", function () {
        setEnabled(root, enable.checked);
      });
      if (enable.checked) setEnabled(root, true);
    }
    function onLenChange(val) {
      var len = clampLen(val, min, max);
      syncLengthUi(root, len);
      if (enable && enable.checked) fill(root, generate(root));
    }
    if (range) range.addEventListener("input", function () { onLenChange(range.value); });
    if (num) num.addEventListener("change", function () { onLenChange(num.value); });
    root.querySelectorAll(".cpn-pw-gen-classes input[type=checkbox]").forEach(function (cb) {
      cb.addEventListener("change", function () {
        if (enable && enable.checked) fill(root, generate(root));
      });
    });
    if (regen) {
      regen.addEventListener("click", function (ev) {
        ev.preventDefault();
        if (enable && !enable.checked) {
          enable.checked = true;
          setEnabled(root, true);
          return;
        }
        fill(root, generate(root));
      });
    }
    if (strongerBtn) {
      strongerBtn.addEventListener("click", function (ev) {
        ev.preventDefault();
        if (enable && !enable.checked) {
          enable.checked = true;
          setEnabled(root, true);
        }
        stronger(root);
      });
    }
    if (copyBtn) {
      copyBtn.addEventListener("click", function (ev) {
        ev.preventDefault();
        var input = findPasswordInput(root);
        var val = input ? input.value : "";
        if (!val) {
          setStatus(root, "Nothing to copy yet. Enable generate or regenerate first.");
          return;
        }
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(val).then(function () {
            setStatus(root, "Copied to clipboard.");
          }).catch(function () {
            setStatus(root, "Could not copy. Select the password field and copy manually.");
          });
        } else {
          input.type = "text";
          input.select();
          try {
            document.execCommand("copy");
            setStatus(root, "Copied to clipboard.");
          } catch (e) {
            setStatus(root, "Could not copy. Select the password field and copy manually.");
          }
        }
      });
    }

    var form = root.closest("form");
    if (form) {
      form.addEventListener("submit", function () {
        var input = findPasswordInput(root);
        if (input && input.value.trim() && enable) {
          enable.checked = false;
        }
      });
    }
  }

  function initAll() {
    document.querySelectorAll(".cpn-pw-gen").forEach(bind);
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initAll);
  } else {
    initAll();
  }
})();
</script>"#
        .to_string()
}

/// Compact CSS for generator controls (dark-mode friendly with existing stack-form vars).
pub fn password_gen_styles() -> String {
    r#"
<style>
.cpn-pw-gen { display:grid; gap:10px; }
.cpn-pw-gen-panel {
  display:grid; gap:10px; padding:12px;
  border:1px solid var(--hairline,#e5e5ea);
  border-radius:8px;
  background:var(--surface-soft,#f6f7f9);
}
.cpn-pw-gen-panel[hidden] { display:none !important; }
.cpn-pw-gen-length-controls {
  display:flex; flex-wrap:wrap; gap:10px; align-items:center;
}
.cpn-pw-gen-length-range { flex:1 1 160px; min-width:120px; max-width:100%; }
.cpn-pw-gen-length-num { width:5.5rem; max-width:100%; }
.cpn-pw-gen-classes {
  display:flex; flex-wrap:wrap; gap:10px 16px;
}
.cpn-pw-gen-class {
  display:inline-flex; align-items:center; gap:6px; margin:0;
  font-weight:500; color:inherit;
}
.cpn-pw-gen-actions {
  display:flex; flex-wrap:wrap; gap:8px;
}
.cpn-pw-gen-actions .btn-secondary { width:auto; }
[data-color-mode="dark"] .cpn-pw-gen-panel,
html[data-color-mode="dark"] .cpn-pw-gen-panel {
  background:var(--surface-soft,#161922);
  border-color:var(--hairline,#2a2f3a);
  color:#f2f4f7;
}
</style>"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_password_requests_server_generate() {
        assert!(wants_server_generated_password(""));
        assert!(wants_server_generated_password("   "));
        assert!(!wants_server_generated_password("Abcdefg1"));
    }

    #[test]
    fn controls_include_regenerate_and_options() {
        let html = password_gen_controls_html();
        assert!(html.contains("cpn-pw-gen-regen"));
        assert!(html.contains("cpn-pw-gen-stronger"));
        assert!(html.contains("cpn-pw-gen-length-range"));
        assert!(html.contains("cpn-pw-gen-upper"));
        assert!(html.contains("cpn-pw-gen-special"));
        assert!(html.contains("Generate a strong password"));
        let script = password_gen_script();
        assert!(script.contains("crypto.getRandomValues"));
        assert!(script.contains("cpn-pw-gen-regen"));
    }

    #[test]
    fn other_accounts_reset_embeds_generator() {
        use crate::account::{default_password_policy, with_test_data_dir};
        use crate::account_mgmt::create_account;
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            create_account(
                "panelowner",
                None,
                true,
                "owner@example.com",
                default_password_policy(),
                "en",
            )
            .expect("create");
            let html = crate::panel_hub_pages_profile::users_self_edit_body_with_tab(
                "panelowner",
                None,
                None,
                None,
                None,
                "other",
                true,
            );
            assert!(html.contains("action=\"/account/users/password\""));
            assert!(html.contains("cpn-pw-gen-regen") && html.contains("cpn-pw-gen-stronger"));
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }
}

//! Uninstall confirmation: impact lists and shared dialog for Plugins / Host packages.
//!
//! Host packages declare impacts in `host_packages_catalog`. Plugins may declare
//! `uninstall_impacts` in `cpn-plugin.json` or catalog `meta.xml`. Built-in maps
//! cover known plugins; unknown packages still get a generic warning and must confirm.

use crate::apps::AppId;
use crate::host_packages_catalog::meta_for;
use crate::plugins_settings::manifest_uninstall_impacts;

/// Accept `confirm=1` (and common truthy aliases) from the confirm dialog.
pub fn confirm_accepted(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "confirm"
    )
}

pub const CONFIRM_REQUIRED_MSG: &str = "Uninstall cancelled: confirmation required. Use the Confirm uninstall dialog (or send confirm=1).";

/// Host package impacts from catalog metadata (always non-empty).
pub fn host_uninstall_impacts(id: AppId) -> Vec<String> {
    meta_for(id)
        .uninstall_impacts
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// Built-in impacts for well-known catalog plugin ids.
pub fn known_plugin_uninstall_impacts(plugin_id: &str) -> Option<Vec<&'static str>> {
    match plugin_id.trim() {
        "fail2ban" => Some(vec![
            "Stops fail2ban.service (and related jail enforcement)",
            "Removes Security > Fail2ban from the panel sidebar",
            "Fail2ban jail management becomes unavailable until reinstalled",
        ]),
        "mtaSts" => Some(vec![
            "Hides Email > MTA-STS until the plugin is installed again",
            "MTA-STS policy publishing / management becomes unavailable",
            "Clears the host mta-sts feature flag when no site still has it enabled",
        ]),
        "bimi" => Some(vec![
            "Hides Email > BIMI until the plugin is installed again",
            "BIMI logo / selector UI becomes unavailable",
            "Clears the host bimi feature flag when no site still has it enabled",
        ]),
        "snappymailWebmail" | "snappymailAdmin" => Some(vec![
            "Removes this site plugin entry and its Settings / Dashboard links",
            "Related SnappyMail sidebar entries for this site may disappear",
            "Does not uninstall the host SnappyMail / Email packages by itself",
        ]),
        "roundcubeWebmail" => Some(vec![
            "Removes this site plugin entry and its Settings links",
            "Roundcube webmail integration for this site becomes unavailable",
        ]),
        _ => None,
    }
}

fn generic_plugin_impacts(plugin_id: &str, plugin_name: &str) -> Vec<String> {
    vec![
        format!(
            "Removes plugin files for `{plugin_id}` under this site's plugins folder"
        ),
        format!(
            "Disables Settings, Dashboard, and sidebar entries for {}",
            if plugin_name.trim().is_empty() {
                plugin_id
            } else {
                plugin_name
            }
        ),
        "Any panel features that depend on this plugin may stop working or hide until it is reinstalled".into(),
        "Exact services for this package were not declared in metadata; review the plugin docs if unsure".into(),
    ]
}

/// Resolve impacts: manifest metadata, then known map, then generic fallback.
pub fn plugin_uninstall_impacts(domain: &str, plugin_id: &str, plugin_name: &str) -> Vec<String> {
    let from_manifest = manifest_uninstall_impacts(domain, plugin_id);
    if !from_manifest.is_empty() {
        return from_manifest;
    }
    if let Some(known) = known_plugin_uninstall_impacts(plugin_id) {
        return known.into_iter().map(str::to_string).collect();
    }
    generic_plugin_impacts(plugin_id, plugin_name)
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Encode impact list for a `data-uninstall-impacts` attribute (JSON array).
pub fn impacts_attr(impacts: &[String]) -> String {
    let json = serde_json::to_string(impacts).unwrap_or_else(|_| {
        "[\"Related services and features may stop working or become unavailable.\"]".into()
    });
    html_escape(&json)
}

/// Shared modal markup (include once on the Plugins hub).
pub fn uninstall_dialog_markup() -> &'static str {
    r#"<dialog id="cpn-uninstall-dialog" class="cpn-uninstall-dialog" aria-labelledby="cpn-uninstall-dialog-title">
  <div class="cpn-uninstall-dialog-inner">
    <h2 id="cpn-uninstall-dialog-title">Confirm uninstall</h2>
    <p id="cpn-uninstall-dialog-lead" class="muted"></p>
    <p class="cpn-uninstall-warn">If you continue, these services and features will stop working or become unavailable:</p>
    <ul id="cpn-uninstall-dialog-impacts"></ul>
    <p class="muted">Cancel leaves everything as it is. Confirm only when you intend to remove this package.</p>
    <div class="cpn-uninstall-actions">
      <button type="button" class="btn-secondary" id="cpn-uninstall-dialog-cancel">Cancel</button>
      <button type="button" class="btn-danger" id="cpn-uninstall-dialog-confirm">Confirm uninstall</button>
    </div>
  </div>
</dialog>"#
}

pub fn uninstall_dialog_styles() -> &'static str {
    r#"
      .cpn-uninstall-dialog {
        border: 1px solid var(--hairline, #cbd5e1);
        border-radius: 14px;
        padding: 0;
        max-width: min(520px, 94vw);
        color: var(--ink, #0f172a);
        background: var(--canvas, #ffffff);
        box-shadow: 0 18px 48px rgba(15, 23, 42, 0.22);
      }
      .cpn-uninstall-dialog::backdrop { background: rgba(15, 23, 42, 0.55); }
      .cpn-uninstall-dialog-inner { margin: 0; padding: 18px 20px 16px; }
      .cpn-uninstall-dialog-inner h2 { margin: 0 0 8px; font-size: 1.15rem; }
      .cpn-uninstall-dialog-inner .muted { margin: 0 0 10px; font-size: 14px; opacity: .88; }
      .cpn-uninstall-warn {
        margin: 0 0 8px; font-size: 14px; font-weight: 600; color: #9a3412;
      }
      [data-color-mode="dark"] .cpn-uninstall-warn { color: #fdba74; }
      #cpn-uninstall-dialog-impacts {
        margin: 0 0 12px; padding-left: 1.2em; font-size: 14px; line-height: 1.45;
      }
      #cpn-uninstall-dialog-impacts li { margin: 0 0 6px; }
      .cpn-uninstall-actions {
        display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; margin-top: 8px;
      }
    "#
}

pub fn uninstall_dialog_script() -> &'static str {
    r#"
<script>
(function () {
  if (window.__cpnUninstallConfirmBound) return;
  window.__cpnUninstallConfirmBound = true;

  function dialogEl() { return document.getElementById('cpn-uninstall-dialog'); }
  function leadEl() { return document.getElementById('cpn-uninstall-dialog-lead'); }
  function listEl() { return document.getElementById('cpn-uninstall-dialog-impacts'); }
  function confirmBtn() { return document.getElementById('cpn-uninstall-dialog-confirm'); }
  function cancelBtn() { return document.getElementById('cpn-uninstall-dialog-cancel'); }

  var pendingForm = null;

  function parseImpacts(raw) {
    if (!raw) return [];
    try {
      var parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) return parsed.map(String);
    } catch (e) {}
    return ['Related services and features may stop working or become unavailable.'];
  }

  function fillDialog(name, impacts) {
    var lead = leadEl();
    var list = listEl();
    if (lead) {
      lead.textContent = 'You are about to uninstall ' + (name || 'this package') + '.';
    }
    if (!list) return;
    list.innerHTML = '';
    impacts.forEach(function (item) {
      var li = document.createElement('li');
      li.textContent = item;
      list.appendChild(li);
    });
  }

  function closeDialog() {
    var d = dialogEl();
    if (d && typeof d.close === 'function') d.close();
    pendingForm = null;
  }

  document.addEventListener('submit', function (ev) {
    var form = ev.target;
    if (!form || !form.classList || !form.classList.contains('cpn-uninstall-form')) return;
    var field = form.querySelector('input[name="confirm"]');
    if (field && String(field.value) === '1') return;
    ev.preventDefault();
    pendingForm = form;
    var name = form.getAttribute('data-uninstall-name') || 'this package';
    var impacts = parseImpacts(form.getAttribute('data-uninstall-impacts'));
    fillDialog(name, impacts);
    var d = dialogEl();
    if (d && typeof d.showModal === 'function') d.showModal();
    else if (window.confirm) {
      var lines = impacts.map(function (i) { return '- ' + i; }).join('\n');
      if (window.confirm('Uninstall ' + name + '?\n\n' + lines)) {
        if (field) field.value = '1';
        form.submit();
      }
      pendingForm = null;
    }
  }, true);

  function onConfirm() {
    if (!pendingForm) return;
    var form = pendingForm;
    var field = form.querySelector('input[name="confirm"]');
    if (field) field.value = '1';
    pendingForm = null;
    var d = dialogEl();
    if (d && typeof d.close === 'function') d.close();
    form.submit();
  }

  document.addEventListener('click', function (ev) {
    var t = ev.target;
    if (!t) return;
    if (t.id === 'cpn-uninstall-dialog-confirm') {
      ev.preventDefault();
      onConfirm();
    } else if (t.id === 'cpn-uninstall-dialog-cancel') {
      ev.preventDefault();
      closeDialog();
    }
  });

  var d = dialogEl();
  if (d) {
    d.addEventListener('cancel', function () { pendingForm = null; });
  }
})();
</script>
"#
}

pub fn uninstall_dialog_bundle() -> String {
    format!(
        "<style>{}</style>\n{}\n{}",
        uninstall_dialog_styles(),
        uninstall_dialog_markup(),
        uninstall_dialog_script()
    )
}

/// Build uninstall form fields shared by plugins and host packages.
pub fn uninstall_form_attrs(package_name: &str, impacts: &[String]) -> String {
    format!(
        r#"class="inline-form cpn-uninstall-form" data-uninstall-name="{name}" data-uninstall-impacts="{impacts}""#,
        name = html_escape(package_name),
        impacts = impacts_attr(impacts),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_flag_truthy() {
        assert!(confirm_accepted("1"));
        assert!(confirm_accepted(" yes "));
        assert!(!confirm_accepted(""));
        assert!(!confirm_accepted("0"));
    }

    #[test]
    fn host_impacts_nonempty() {
        for id in AppId::all() {
            assert!(
                !host_uninstall_impacts(*id).is_empty(),
                "missing impacts for {}",
                id.as_str()
            );
        }
    }

    #[test]
    fn known_fail2ban_impacts() {
        let impacts = known_plugin_uninstall_impacts("fail2ban").unwrap();
        assert!(impacts.iter().any(|s| s.contains("fail2ban")));
    }

    #[test]
    fn generic_fallback_mentions_plugin() {
        let impacts = plugin_uninstall_impacts("", "demoPlugin", "Demo Plugin");
        assert!(impacts.iter().any(|s| s.contains("demoPlugin")));
    }
}

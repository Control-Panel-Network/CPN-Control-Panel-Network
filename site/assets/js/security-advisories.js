(function () {
  "use strict";

  var DATA_URL = "data/security-advisories.json";

  function escapeHtml(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function formatPublished(iso) {
    if (!iso || typeof iso !== "string") {
      return "n/a";
    }
    var match = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso.trim());
    if (!match) {
      return escapeHtml(iso);
    }
    return match[3] + "/" + match[2] + "/" + match[1];
  }

  function formatIds(entry) {
    var parts = [];
    if (entry.cve) {
      parts.push(escapeHtml(entry.cve));
    }
    if (entry.ghsa) {
      parts.push(escapeHtml(entry.ghsa));
    }
    return parts.length ? parts.join("<br>") : "none";
  }

  function severityClass(severity) {
    var key = String(severity || "").toLowerCase();
    if (key === "critical" || key === "high" || key === "medium" || key === "low") {
      return "sev-" + key;
    }
    return "sev-unknown";
  }

  function renderReferences(refs) {
    if (!Array.isArray(refs) || !refs.length) {
      return "<p class=\"muted\">No references listed.</p>";
    }
    var items = refs.map(function (url) {
      var safe = escapeHtml(url);
      return "<li><a href=\"" + safe + "\" rel=\"noopener noreferrer\">" + safe + "</a></li>";
    });
    return "<ul class=\"ref-list\">" + items.join("") + "</ul>";
  }

  function renderDetails(list) {
    return list.map(function (entry) {
      var id = escapeHtml(entry.id || "unknown");
      var title = escapeHtml(entry.title || "Untitled");
      return (
        "<article class=\"advisory-card\" id=\"" + id + "\">" +
        "<h3>" + id + ": " + title + "</h3>" +
        "<dl class=\"advisory-meta\">" +
        "<div><dt>CVE</dt><dd>" + escapeHtml(entry.cve || "none") + "</dd></div>" +
        "<div><dt>GHSA</dt><dd>" + escapeHtml(entry.ghsa || "none") + "</dd></div>" +
        "<div><dt>Severity</dt><dd><span class=\"sev-badge " + severityClass(entry.severity) + "\">" +
        escapeHtml(entry.severity || "unknown") + "</span></dd></div>" +
        "<div><dt>Affected</dt><dd>" + escapeHtml(entry.affected_versions || "n/a") + "</dd></div>" +
        "<div><dt>Fixed in</dt><dd>" + escapeHtml(entry.fixed_version || "n/a") + "</dd></div>" +
        "<div><dt>Published</dt><dd>" + formatPublished(entry.published) + "</dd></div>" +
        "</dl>" +
        "<p><strong>Summary:</strong> " + escapeHtml(entry.summary || "") + "</p>" +
        "<p><strong>Impact:</strong> " + escapeHtml(entry.impact || "") + "</p>" +
        "<p><strong>Remediation:</strong> " + escapeHtml(entry.remediation || "") + "</p>" +
        "<p><strong>References</strong></p>" +
        renderReferences(entry.references) +
        "</article>"
      );
    }).join("");
  }

  function renderTable(list) {
    return list.map(function (entry) {
      var id = escapeHtml(entry.id || "unknown");
      return (
        "<tr>" +
        "<td><a href=\"#" + id + "\">" + id + "</a></td>" +
        "<td>" + formatIds(entry) + "</td>" +
        "<td><span class=\"sev-badge " + severityClass(entry.severity) + "\">" +
        escapeHtml(entry.severity || "unknown") + "</span></td>" +
        "<td>" + escapeHtml(entry.affected_versions || "n/a") + "</td>" +
        "<td>" + escapeHtml(entry.fixed_version || "n/a") + "</td>" +
        "<td>" + formatPublished(entry.published) + "</td>" +
        "<td>" + escapeHtml(entry.summary || "") + "</td>" +
        "</tr>"
      );
    }).join("");
  }

  function showEmpty(statusEl, emptyEl, tableWrap, detailsEl) {
    statusEl.hidden = true;
    emptyEl.hidden = false;
    tableWrap.hidden = true;
    detailsEl.hidden = true;
    detailsEl.innerHTML = "";
  }

  function showList(statusEl, emptyEl, tableWrap, detailsEl, tbody, list) {
    statusEl.hidden = true;
    emptyEl.hidden = true;
    tableWrap.hidden = false;
    detailsEl.hidden = false;
    tbody.innerHTML = renderTable(list);
    detailsEl.innerHTML = renderDetails(list);
  }

  function showError(statusEl, emptyEl, tableWrap, detailsEl, message) {
    statusEl.hidden = false;
    statusEl.textContent = message;
    emptyEl.hidden = true;
    tableWrap.hidden = true;
    detailsEl.hidden = true;
  }

  function init() {
    var statusEl = document.getElementById("advisories-status");
    var emptyEl = document.getElementById("advisories-empty");
    var tableWrap = document.getElementById("advisories-table-wrap");
    var detailsEl = document.getElementById("advisories-details");
    var tbody = document.getElementById("advisories-tbody");

    if (!statusEl || !emptyEl || !tableWrap || !detailsEl || !tbody) {
      return;
    }

    fetch(DATA_URL, { credentials: "same-origin", cache: "no-cache" })
      .then(function (response) {
        if (!response.ok) {
          throw new Error("HTTP " + response.status);
        }
        return response.json();
      })
      .then(function (payload) {
        var list = Array.isArray(payload)
          ? payload
          : (payload && Array.isArray(payload.advisories) ? payload.advisories : null);

        if (!list) {
          throw new Error("Invalid advisories payload");
        }

        if (!list.length) {
          showEmpty(statusEl, emptyEl, tableWrap, detailsEl);
          return;
        }

        showList(statusEl, emptyEl, tableWrap, detailsEl, tbody, list);
      })
      .catch(function () {
        showError(
          statusEl,
          emptyEl,
          tableWrap,
          detailsEl,
          "Could not load advisories. See the GitHub security advisories log linked above, or try again later."
        );
      });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();

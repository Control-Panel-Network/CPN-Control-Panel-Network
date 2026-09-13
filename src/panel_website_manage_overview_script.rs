//! Client script for Manage Overview host metrics charts (live poll + minimalist snapshot).

pub fn overview_metrics_script() -> &'static str {
    r#"<script>
(function () {
  var root = document.querySelector(".manage-charts[data-metrics-domain]");
  if (!root) return;
  var domain = root.getAttribute("data-metrics-domain") || "";
  var pollMs = parseInt(root.getAttribute("data-metrics-poll") || "10000", 10);
  var dialog = document.getElementById("manage-metric-dialog");
  var titleEl = document.getElementById("manage-metric-dialog-title");
  var summaryEl = document.getElementById("manage-metric-dialog-summary");
  var svgEl = document.getElementById("manage-metric-dialog-svg");
  var statsEl = document.getElementById("manage-metric-dialog-stats");
  var dialogReadout = document.getElementById("manage-metric-dialog-readout");
  var lastPayload = null;
  var selectedTs = { cpu: null, mem: null };
  var dialogKind = null;

  function pct(v) {
    return (v === null || v === undefined) ? "n/a" : (Math.round(v) + "%");
  }

  function seedSnapshot() {
    var el = document.getElementById("manage-metrics-snapshot");
    if (!el) return;
    try { lastPayload = JSON.parse(el.textContent || ""); } catch (e) {}
  }

  function setReadout(readout, clock, valuePct, selected) {
    if (!readout) return;
    var hint = readout.querySelector(".manage-chart-readout-hint");
    var value = readout.querySelector(".manage-chart-readout-value");
    if (!value) return;
    if (!selected || !clock) {
      readout.classList.remove("is-active");
      value.hidden = true;
      value.textContent = "";
      if (hint) hint.hidden = false;
      return;
    }
    readout.classList.add("is-active");
    if (hint) hint.hidden = true;
    value.hidden = false;
    value.textContent = clock + "  " + valuePct + "%";
  }

  function clearHighlight(svg) {
    if (!svg) return;
    svg.classList.remove("has-selection");
    svg.querySelectorAll(".metric-hit.is-selected, .metric-dot.is-selected").forEach(function (el) {
      el.classList.remove("is-selected");
      if (el.classList.contains("metric-dot")) el.setAttribute("r", "3.2");
    });
    var cross = svg.querySelector(".metric-crosshair");
    if (cross) cross.setAttribute("opacity", "0");
  }

  function applySelection(svg, readout, ts) {
    if (!svg) return;
    clearHighlight(svg);
    if (ts === null || ts === undefined) {
      setReadout(readout, null, null, false);
      return;
    }
    var hit = svg.querySelector('.metric-hit[data-t="' + ts + '"]');
    if (!hit) {
      setReadout(readout, null, null, false);
      return;
    }
    hit.classList.add("is-selected");
    var dot = svg.querySelector('.metric-dot[data-t="' + ts + '"]');
    if (dot) {
      dot.classList.add("is-selected");
      dot.setAttribute("r", "5");
    }
    svg.classList.add("has-selection");
    var cross = svg.querySelector(".metric-crosshair");
    if (cross) {
      var cx = hit.getAttribute("cx") || "0";
      cross.setAttribute("x1", cx);
      cross.setAttribute("x2", cx);
      cross.setAttribute("opacity", "0.85");
    }
    setReadout(readout, hit.getAttribute("data-clock"), hit.getAttribute("data-pct"), true);
  }

  function bindHits(scope, kind, readout) {
    if (!scope) return;
    scope.querySelectorAll(".metric-hit").forEach(function (hit) {
      function selectHit(ev) {
        if (ev) { ev.preventDefault(); ev.stopPropagation(); }
        var ts = hit.getAttribute("data-t");
        var hitKind = hit.getAttribute("data-kind") || kind;
        if (hitKind === "cpu" || hitKind === "mem") selectedTs[hitKind] = ts;
        applySelection(scope.querySelector("svg.manage-metric-svg") || scope, readout, ts);
      }
      hit.addEventListener("click", selectHit);
      hit.addEventListener("mouseenter", function () {
        if (window.matchMedia && window.matchMedia("(hover: hover)").matches) selectHit(null);
      });
    });
  }

  function wireInlineCharts() {
    ["cpu", "mem"].forEach(function (kind) {
      var wrap = root.querySelector('[data-metric-svg="' + kind + '"]');
      var readout = root.querySelector('[data-metric-readout="' + kind + '"]');
      if (!wrap) return;
      bindHits(wrap, kind, readout);
      applySelection(wrap.querySelector("svg"), readout, selectedTs[kind]);
    });
  }

  function applyPayload(data) {
    lastPayload = data;
    if (!data || !data.ok) return;
    ["cpu", "mem"].forEach(function (kind) {
      var block = data[kind];
      if (!block) return;
      var cur = root.querySelector('[data-metric-current="' + kind + '"]');
      var avg = root.querySelector('[data-metric-avg="' + kind + '"]');
      var peak = root.querySelector('[data-metric-peak="' + kind + '"]');
      var svgWrap = root.querySelector('[data-metric-svg="' + kind + '"]');
      if (cur) cur.textContent = pct(block.current);
      if (avg) avg.textContent = pct(block.avg);
      if (peak) peak.textContent = pct(block.peak);
      if (svgWrap && block.svg) svgWrap.innerHTML = block.svg;
    });
    wireInlineCharts();
    if (dialog && dialog.open && dialogKind && data[dialogKind] && svgEl) {
      svgEl.innerHTML = data[dialogKind].svg || "";
      bindHits(svgEl, dialogKind, dialogReadout);
      applySelection(svgEl.querySelector("svg"), dialogReadout, selectedTs[dialogKind]);
    }
    var bwHint = document.querySelector(".manage-stat-hint");
    if (data.bandwidth && bwHint) {
      var card = bwHint.closest(".manage-stat");
      if (card) {
        var strong = card.querySelector("strong");
        if (strong && data.bandwidth.label) strong.textContent = data.bandwidth.label;
      }
      if (data.bandwidth.hint) {
        bwHint.textContent = data.bandwidth.hint;
        if (card) card.setAttribute("title", data.bandwidth.hint);
      }
    }
  }

  function openMetric(kind) {
    if (!dialog || !lastPayload || !lastPayload[kind]) return;
    dialogKind = kind;
    var block = lastPayload[kind];
    var name = kind === "mem" ? "Memory Usage (host)" : "CPU Usage (host)";
    if (titleEl) titleEl.textContent = name;
    if (summaryEl) {
      var sampleNote = lastPayload.minimalist
        ? " Snapshot (minimalist); refresh the page to update."
        : " Samples every ~10s (real poll points, not per-minute).";
      summaryEl.textContent = (lastPayload.detail || "Host live metrics.") +
        " Window: last " + Math.round((lastPayload.window_seconds || 900) / 60) +
        " minutes." + sampleNote;
    }
    if (svgEl) svgEl.innerHTML = block.svg || "";
    if (statsEl) {
      statsEl.innerHTML =
        "<li>Current: <strong>" + pct(block.current) + "</strong></li>" +
        "<li>Average: <strong>" + pct(block.avg) + "</strong></li>" +
        "<li>Peak: <strong>" + pct(block.peak) + "</strong></li>" +
        "<li>Samples: <strong>" + ((lastPayload.samples && lastPayload.samples.length) || 0) + "</strong></li>";
    }
    bindHits(svgEl, kind, dialogReadout);
    applySelection(svgEl && svgEl.querySelector("svg"), dialogReadout, selectedTs[kind]);
    if (typeof dialog.showModal === "function") dialog.showModal();
  }

  function poll() {
    fetch("/api/websites/manage/metrics?domain=" + encodeURIComponent(domain), {
      credentials: "same-origin",
      headers: { "Accept": "application/json" }
    }).then(function (r) { return r.json(); }).then(applyPayload).catch(function () {});
  }

  root.querySelectorAll("[data-metric-details]").forEach(function (btn) {
    btn.addEventListener("click", function (ev) {
      ev.preventDefault();
      openMetric(btn.getAttribute("data-metric-details") || "cpu");
    });
  });

  if (dialog) {
    dialog.addEventListener("close", function () { dialogKind = null; });
  }

  seedSnapshot();
  wireInlineCharts();
  if (pollMs > 0) {
    poll();
    setInterval(poll, pollMs);
  }
})();
</script>"#
}

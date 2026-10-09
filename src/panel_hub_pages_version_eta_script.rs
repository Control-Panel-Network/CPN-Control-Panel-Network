//! Version page ETA JS: estimated time remaining for upgrade / repair jobs.
//!
//! Hybrid estimate. Each known stage (download, package install, npm,
//! cargo build, verify) carries a typical "seconds left from here" hint.
//! That hint is blended with a linear rate estimate (elapsed * remaining
//! percent / done percent). Hints dominate early, when the rate is noisy
//! and a long cargo build may still be ahead; the rate dominates late.
//! The result is smoothed and rounded so the label never fakes precision.

/// Inline JS (same IIFE as Version Management). Pure helpers plus one painter.
pub fn version_page_eta_script() -> &'static str {
    r##"
  var ETA_MIN_ELAPSED_SECS = 5;
  var ETA_MIN_PCT_FOR_RATE = 3;
  var ETA_MIN_ELAPSED_FOR_RATE = 8;
  var ETA_STAGES = [
    { key: "toolchain", re: /rustup|rust toolchain/i, secs: 1500 },
    { key: "cargo", re: /cargo|building panel|building installer binary|compil/i, secs: 1200 },
    { key: "npm", re: /npm|installer ui/i, secs: 1200 },
    { key: "extract", re: /extract/i, secs: 1300 },
    { key: "commit-bin", re: /commit binar|github actions/i, secs: 600 },
    { key: "commit-ref", re: /resolving commit/i, secs: 700 },
    { key: "pkg-install", re: /installing rpm|installing deb|replacing cpn-installer/i, secs: 120 },
    { key: "docker", re: /docker/i, secs: 90 },
    { key: "verify-bin", re: /cleaning|verifying installer binary/i, secs: 60 },
    { key: "verify-svc", re: /verifying services|migrations/i, secs: 30 },
    { key: "download", re: /sha256sums|downloading|preparing/i, secs: 180 }
  ];
  var ETA_PHASE_FALLBACK = {
    downloading: 180,
    installing: 120,
    testing: 60,
    verifying: 30,
    configuring: 60
  };
  var etaState = { startedAt: 0, stageKey: "", stageStartedAt: 0, smoothed: null };
  var etaEl = document.getElementById("cpn-version-progress-eta");
  function etaClamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)); }
  function etaReset(nowMs) {
    etaState.startedAt = nowMs || Date.now();
    etaState.stageKey = "";
    etaState.stageStartedAt = etaState.startedAt;
    etaState.smoothed = null;
  }
  function etaStageFor(phase, message) {
    var msg = String(message || "");
    for (var i = 0; i < ETA_STAGES.length; i++) {
      if (ETA_STAGES[i].re.test(msg)) return ETA_STAGES[i];
    }
    var ph = String(phase || "");
    if (Object.prototype.hasOwnProperty.call(ETA_PHASE_FALLBACK, ph)) {
      return { key: "phase:" + ph, secs: ETA_PHASE_FALLBACK[ph] };
    }
    return null;
  }
  function etaCompute(pct, elapsedSecs, phase, message, nowMs) {
    var now = nowMs || Date.now();
    var p = etaClamp(Number(pct) || 0, 0, 100);
    var elapsed = Math.max(0, Number(elapsedSecs) || 0);
    if (p >= 100) return 0;
    var stage = etaStageFor(phase, message);
    var key = stage ? stage.key : ("raw:" + phase + "|" + message);
    if (key !== etaState.stageKey) {
      etaState.stageKey = key;
      etaState.stageStartedAt = now;
      etaState.smoothed = null;
    }
    if (elapsed < ETA_MIN_ELAPSED_SECS) return null;
    var inStage = Math.max(0, (now - etaState.stageStartedAt) / 1000);
    var hint = null;
    if (stage) hint = Math.max(stage.secs - inStage, stage.secs * 0.15);
    var rate = null;
    if (p >= ETA_MIN_PCT_FOR_RATE && elapsed >= ETA_MIN_ELAPSED_FOR_RATE) {
      rate = elapsed * (100 - p) / p;
    }
    var est;
    if (hint != null && rate != null) {
      var w = etaClamp(1 - p / 100, 0.2, 0.85);
      est = w * hint + (1 - w) * rate;
    } else if (hint != null) {
      est = hint;
    } else if (rate != null) {
      est = rate;
    } else {
      return null;
    }
    if (etaState.smoothed == null) etaState.smoothed = est;
    else etaState.smoothed = etaState.smoothed * 0.65 + est * 0.35;
    return Math.max(0, Math.round(etaState.smoothed));
  }
  function etaFormat(secs) {
    if (secs == null || !isFinite(secs)) return "";
    var s = Math.max(0, Math.round(Number(secs)));
    if (s < 10) return "Almost done";
    if (s < 60) return "About " + (Math.round(s / 5) * 5) + " sec left";
    if (s < 3600) return "About " + Math.ceil(s / 60) + " min left";
    var hrs = Math.floor(s / 3600);
    var mins = Math.round(((s % 3600) / 60) / 5) * 5;
    if (mins >= 60) { hrs += 1; mins = 0; }
    return "About " + hrs + " hr" + (mins ? (" " + mins + " min") : "") + " left";
  }
  function etaSetText(text, secs) {
    if (!etaEl) return;
    etaEl.textContent = text || "";
    if (secs == null) etaEl.removeAttribute("data-eta-secs");
    else etaEl.setAttribute("data-eta-secs", String(secs));
    etaEl.style.display = text ? "" : "none";
  }
  function etaClear() { etaSetText("", null); }
  function etaElapsedFrom(st, nowMs) {
    var now = nowMs || Date.now();
    var server = st && st.job_elapsed_secs;
    if (server != null && isFinite(Number(server))) {
      var e = Math.max(0, Number(server));
      etaState.startedAt = now - (e * 1000);
      return e;
    }
    if (!etaState.startedAt) etaState.startedAt = now;
    return Math.max(0, (now - etaState.startedAt) / 1000);
  }
  function etaResume(st) {
    etaReset(Date.now());
    etaElapsedFrom(st, Date.now());
  }
  function etaPaint(st, pct) {
    if (!st || !st.busy || st.phase === "completed" || st.phase === "failed") {
      etaClear();
      return "";
    }
    var now = Date.now();
    var elapsed = etaElapsedFrom(st, now);
    var secs = etaCompute(pct, elapsed, st.phase, st.message, now);
    if (secs == null) {
      etaSetText("Estimating time left...", null);
      return "Estimating time left...";
    }
    var text = etaFormat(secs);
    etaSetText(text, secs);
    return text;
  }
"##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eta_script_has_hybrid_estimate_and_friendly_copy() {
        let js = version_page_eta_script();
        assert!(js.contains("etaCompute"));
        assert!(js.contains("etaFormat"));
        assert!(js.contains("etaPaint"));
        assert!(js.contains("etaResume"));
        assert!(js.contains("etaClear"));
        assert!(js.contains("job_elapsed_secs"));
        assert!(js.contains("cpn-version-progress-eta"));
        assert!(js.contains("Estimating time left..."));
        assert!(js.contains("sec left"));
        assert!(js.contains("min left"));
        assert!(js.contains("cargo"));
        assert!(js.contains("npm"));
        assert!(js.contains("installing rpm|installing deb"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

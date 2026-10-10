//! Version page picker/render/check JS (same IIFE as Version Management).

pub fn version_page_ui_script() -> &'static str {
    r##"
  function hideResults() {
    if (!resultsEl) return;
    resultsEl.style.display = "none";
    resultsEl.innerHTML = "";
    if (searchEl) searchEl.setAttribute("aria-expanded", "false");
  }
  function showResults(items) {
    if (!resultsEl) return;
    resultsEl.innerHTML = "";
    if (!items.length) {
      var empty = document.createElement("li");
      empty.className = "muted";
      empty.style.padding = "8px 10px";
      empty.textContent = "No matching releases";
      resultsEl.appendChild(empty);
      resultsEl.style.display = "block";
      if (searchEl) searchEl.setAttribute("aria-expanded", "true");
      return;
    }
    items.forEach(function (item) {
      var li = document.createElement("li");
      li.setAttribute("role", "option");
      li.tabIndex = -1;
      li.style.padding = "8px 10px";
      li.style.cursor = "pointer";
      li.style.borderBottom = "1px solid rgba(148,163,184,0.15)";
      li.dataset.tag = item.tag;
      li.textContent = item.label;
      li.addEventListener("mousedown", function (ev) {
        ev.preventDefault();
        setSelected(item.tag);
        hideResults();
      });
      resultsEl.appendChild(li);
    });
    resultsEl.style.display = "block";
    if (searchEl) searchEl.setAttribute("aria-expanded", "true");
  }
  function filterReleases(query) {
    var q = String(query || "").trim().toLowerCase();
    var installed = infoCache ? norm(infoCache.installed_version) : "";
    var latest = infoCache ? norm(infoCache.latest_version || (infoCache.latest_tag || "")) : "";
    var out = [];
    releaseCache.forEach(function (r) {
      var ver = r.version || norm(r.tag_name);
      var tag = r.tag_name || ver;
      var hay = (tag + " " + ver).toLowerCase();
      if (q && hay.indexOf(q) === -1) return;
      var marks = [];
      if (norm(ver) === installed || norm(tag) === installed) marks.push("installed");
      if (latest && (norm(ver) === latest || norm(tag) === latest)) marks.push("latest");
      if (isOutsideSupport(tag)) marks.push("outside support");
      var dateLabel = r.published_at ? (" · " + formatPublishedAt(r.published_at)) : "";
      out.push({
        tag: tag,
        label: tag + dateLabel + (marks.length ? " (" + marks.join(", ") + ")" : "")
      });
    });
    return out;
  }
  function fillPicker(info) {
    releaseCache = info.releases || [];
    var installed = norm(info.installed_version);
    var latest = norm(info.latest_version || (info.latest_tag || ""));
    if (!releaseCache.length) {
      setSelected("");
      if (searchEl) searchEl.placeholder = "No releases loaded";
      return;
    }
    var prefer = "";
    if (latest) {
      for (var i = 0; i < releaseCache.length; i++) {
        var ver = releaseCache[i].version || norm(releaseCache[i].tag_name);
        var tag = releaseCache[i].tag_name || ver;
        if (norm(ver) === latest || norm(tag) === latest) {
          prefer = tag;
          break;
        }
      }
    }
    if (!prefer && installed) {
      for (var j = 0; j < releaseCache.length; j++) {
        var ver2 = releaseCache[j].version || norm(releaseCache[j].tag_name);
        var tag2 = releaseCache[j].tag_name || ver2;
        if (norm(ver2) === installed || norm(tag2) === installed) {
          prefer = tag2;
          break;
        }
      }
    }
    if (!prefer && releaseCache[0]) {
      prefer = releaseCache[0].tag_name || releaseCache[0].version || "";
    }
    setSelected(prefer);
    hideResults();
  }
  function render(info) {
    infoCache = info;
    if (!info) {
      stopRetryCountdown();
      if (statusEl) statusEl.textContent = "Could not load version information.";
      return;
    }
    saveVersionCache(info);
    paintIdentityRows(info);
    if (runningEl && info.running_version) runningEl.textContent = info.running_version;
    if (installedEl && info.installed_version) installedEl.textContent = info.installed_version;
    var hasReleaseOrTip = !!(info.latest_version || info.latest_tag || info.stable_tip_sha
      || (info.releases && info.releases.length));
    if (hasReleaseOrTip || info.installed_package_color) {
      paintInstalledVersionColor(resolveInstalledColor(info));
      var currentStrong = document.getElementById("cpn-version-current");
      if (currentStrong) {
        var colorState = resolveInstalledColor(info);
        if (colorState === "stale_behind" || colorState === "stale") {
          currentStrong.style.color = "#f87171";
          currentStrong.setAttribute("data-update-state", "stale");
        } else if (colorState === "update_available" || colorState === "behind") {
          currentStrong.style.color = "#fb923c";
          currentStrong.setAttribute("data-update-state", "behind");
        } else if (colorState === "current") {
          currentStrong.style.color = "#4ade80";
          currentStrong.setAttribute("data-update-state", "current");
        } else {
          currentStrong.style.color = "";
          currentStrong.removeAttribute("data-update-state");
        }
      }
    } else {
      paintInstalledVersionColor(null);
    }
    var latestLabel = info.latest_version || norm(info.latest_tag) || "-";
    var latestTag = info.latest_tag ? String(info.latest_tag) : "";
    if (latestEl) {
      if (latestTag && norm(latestTag) !== norm(latestLabel) && latestLabel !== "-") {
        latestEl.textContent = latestLabel + " (" + latestTag + ")";
      } else {
        latestEl.textContent = latestLabel !== "-" ? latestLabel : (latestTag || "-");
      }
    }
    var tipShort = info.stable_tip_short || (info.stable_tip_sha
      ? String(info.stable_tip_sha).substring(0, 7) : "");
    var branchName = info.stable_branch ? String(info.stable_branch) : "stable";
    var tipLine = "";
    if (info.stable_update_available || (tipShort && info.running_sha
      && String(info.running_sha).substring(0, 7) !== tipShort)) {
      tipLine = "Latest commits: " + branchName
        + (tipShort ? (" @ " + tipShort) : "");
    }
    var stableTipEl = document.getElementById("cpn-version-stable-tip");
    if (stableTipEl) stableTipEl.textContent = tipLine;
    if (sourceTipEl) {
      var srcBits = [];
      if (info.using_fork) srcBits.push("fork");
      else srcBits.push("official");
      if (info.repo) srcBits.push(info.repo);
      srcBits.push("branch " + branchName + (branchName === "stable" ? "" : " (lab)"));
      sourceTipEl.textContent = srcBits.join(" · ") || "-";
    }
    if (upstreamTipEl) {
      if (info.using_fork) {
        var up = info.upstream_latest_version || norm(info.upstream_latest_tag) || "";
        upstreamTipEl.textContent = up
          ? ("Upstream official: " + up
            + (info.upstream_repo ? (" (" + info.upstream_repo + ")") : ""))
          : (info.upstream_repo ? ("Upstream: " + info.upstream_repo) : "");
      } else {
        upstreamTipEl.textContent = "";
      }
    }
    var hasTip = !!(info.latest_version || info.latest_tag || (info.releases && info.releases.length));
    var wait = Math.max(0, Math.floor(Number(info.retry_after_secs) || 0));
    var liveRetry = wait > 0;
    if (liveRetry) {
      startRetryCountdown(wait);
    } else {
      stopRetryCountdown();
      if (!statusEl) {
        // no status node
      } else if (info.check_error && !hasTip) {
        statusEl.textContent = "Update check failed: " + info.check_error;
      } else if (info.rate_limited && info.cache_note) {
        statusEl.textContent = info.cache_note;
      } else if (info.update_available) {
        if (info.stable_update_available && !info.release_update_available) {
          statusEl.textContent = "Update available: " + branchName + " commits are ahead of this build.";
        } else if (info.stable_update_available && info.release_update_available) {
          statusEl.textContent = "Update available: newer release and " + branchName + " commits.";
        } else {
          statusEl.textContent = "Update available: newer release listed above.";
        }
      } else if (info.cache_note) {
        statusEl.textContent = info.cache_note;
      } else if (info.check_error && hasTip) {
        statusEl.textContent = "Showing release info. Note: " + info.check_error;
      } else {
        statusEl.textContent = "Up to date with the latest known release.";
      }
    }
    paintDetailRows(info, liveRetry, hasTip);
    fillPicker(info);
    paintReleaseDates(info);
  }
  function check(forceRefresh) {
    if (forceRefresh && retryLeft > 0) return;
    if (statusEl) {
      statusEl.textContent = forceRefresh
        ? "Refreshing (keeping current values)..."
        : (infoCache ? "Updating status..." : "Checking for updates...");
    }
    if (btn) {
      btn.disabled = true;
      btn.setAttribute("aria-busy", "true");
    }
    var url = "/api/version-check" + (forceRefresh ? "?refresh=1" : "");
    fetch(url, {
      credentials: "same-origin",
      headers: { "Accept": "application/json" },
      cache: "no-store"
    }).then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }).then(function (info) {
      render(info);
      if (btn) btn.removeAttribute("aria-busy");
      syncRefreshButton();
    }).catch(function (err) {
      stopRetryCountdown();
      if (statusEl) statusEl.textContent = friendlyFetchError(err, "Update check");
      if (latestEl && (!latestEl.textContent || latestEl.textContent === "Loading...")) {
        latestEl.textContent = "-";
      }
      if (sourceTipEl && (!sourceTipEl.textContent || sourceTipEl.textContent === "Loading...")) {
        sourceTipEl.textContent = "-";
      }
      if (btn) btn.removeAttribute("aria-busy");
      syncRefreshButton();
    });
  }
"##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_script_renders_and_checks() {
        let js = version_page_ui_script();
        assert!(js.contains("function render"));
        assert!(js.contains("function check"));
        assert!(js.contains("fillPicker"));
        assert!(js.contains("\"Latest commits: \" + branchName"));
        assert!(js.contains("branchName + \" commits are ahead of this build.\""));
        assert!(js.contains("\"branch \" + branchName"));
        assert!(!js.contains("Stable commits:"));
        assert!(!js.contains('\u{2014}'));
        assert!(!js.contains('\u{2013}'));
    }
}

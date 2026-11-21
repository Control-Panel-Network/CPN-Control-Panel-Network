(function () {
  "use strict";

  function setCopied(button) {
    button.dataset.copied = "1";
    button.textContent = "Copied";
    window.setTimeout(function () {
      button.dataset.copied = "0";
      button.textContent = "Copy";
    }, 1600);
  }

  function copyText(text, button) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(function () {
        setCopied(button);
      }).catch(function () {
        fallbackCopy(text, button);
      });
      return;
    }
    fallbackCopy(text, button);
  }

  function fallbackCopy(text, button) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.left = "-9999px";
    document.body.appendChild(area);
    area.select();
    try {
      document.execCommand("copy");
      setCopied(button);
    } catch (err) {
      button.textContent = "Select";
    }
    document.body.removeChild(area);
  }

  document.querySelectorAll("[data-copy-target]").forEach(function (button) {
    button.addEventListener("click", function () {
      var id = button.getAttribute("data-copy-target");
      var node = id ? document.getElementById(id) : null;
      if (!node) {
        return;
      }
      copyText(node.textContent.replace(/\s+/g, " ").trim(), button);
    });
  });

  function applyRelease(version, tag) {
    if (!version) {
      return;
    }
    var displayTag = tag || ("v" + version);
    document.querySelectorAll(".js-cpn-version").forEach(function (node) {
      node.textContent = version;
    });
    document.querySelectorAll(".js-cpn-tag").forEach(function (node) {
      node.textContent = displayTag;
    });
    var pin = document.getElementById("cmd-pin");
    if (pin) {
      pin.textContent =
        "curl -fsSL https://cpn.newstargeted.com/upgrade.sh | bash -s -- -b " + displayTag;
    }
  }

  function fetchReleaseFromApi(apiUrl) {
    return fetch(apiUrl, { credentials: "same-origin", cache: "no-store" })
      .then(function (res) {
        if (!res.ok) {
          throw new Error("api " + res.status);
        }
        return res.json();
      })
      .then(function (data) {
        if (data && data.version) {
          applyRelease(data.version, data.tag);
        }
      });
  }

  function fetchReleaseFromGitHub() {
    var githubUrl =
      "https://api.github.com/repos/Control-Panel-Network/CPN-Control-Panel-Network/releases/latest";
    return fetch(githubUrl, {
      headers: { Accept: "application/vnd.github+json" },
      cache: "no-store",
    })
      .then(function (res) {
        if (!res.ok) {
          throw new Error("github " + res.status);
        }
        return res.json();
      })
      .then(function (data) {
        if (data && data.tag_name) {
          var tag = String(data.tag_name);
          var version = tag.replace(/^v/i, "");
          applyRelease(version, tag);
        }
      });
  }

  var body = document.body;
  var apiPath = body ? body.getAttribute("data-cpn-release-api") : null;
  if (apiPath && apiPath.indexOf("__CPN_") === -1) {
    fetchReleaseFromApi(apiPath).catch(function () {
      return fetchReleaseFromGitHub();
    });
  } else if (document.querySelector(".js-cpn-version")) {
    fetchReleaseFromGitHub().catch(function () {
      /* static / PHP-rendered fallback already in the page */
    });
  }
})();
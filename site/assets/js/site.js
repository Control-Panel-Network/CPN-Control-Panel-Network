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
})();

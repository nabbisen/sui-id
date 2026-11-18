// sui-id password-match indicator (RFC 144 N2e §2).
//
// Compares two password fields already in the page and toggles which of
// two server-rendered messages a status element shows. The comparison
// happens here, in the page: nothing is sent, stored or logged, and no
// password text is written anywhere -- not into the DOM, a data-
// attribute, the console or storage. It reveals only match or not, which
// the person typing already knows. Without this script every form
// behaves exactly as it did before: the server's own check is the one
// that decides, unconditionally and unchanged.
//
// No page knowledge: a confirmation input names the field it confirms
// with data-confirms="<id>", and lists its status element among its
// aria-describedby ids (a space-separated list, which may also name a
// hint): the status element is the one carrying data-match-text. It holds both
// messages, already in the page's language, as data-match-text and
// data-mismatch-text -- this script switches which one shows and
// nothing else.
(function () {
  "use strict";

  function wire(confirmInput) {
    var targetId = confirmInput.getAttribute("data-confirms");
    var target = targetId ? document.getElementById(targetId) : null;
    var describedBy = (confirmInput.getAttribute("aria-describedby") || "").split(/\s+/);
    var status = null;
    for (var j = 0; j < describedBy.length && !status; j++) {
      var candidate = describedBy[j] ? document.getElementById(describedBy[j]) : null;
      if (candidate && candidate.hasAttribute("data-match-text")) {
        status = candidate;
      }
    }
    if (!target || !status) {
      return;
    }
    var matchText = status.getAttribute("data-match-text") || "";
    var mismatchText = status.getAttribute("data-mismatch-text") || "";

    function update() {
      var confirmed = confirmInput.value;
      var original = target.value;
      if (confirmed.length === 0) {
        status.hidden = true;
        status.textContent = "";
        return;
      }
      var matches = confirmed === original;
      status.hidden = false;
      status.textContent = matches ? matchText : mismatchText;
    }

    confirmInput.addEventListener("input", update);
    target.addEventListener("input", update);
  }

  var confirms = document.querySelectorAll("[data-confirms]");
  for (var i = 0; i < confirms.length; i++) {
    wire(confirms[i]);
  }
})();

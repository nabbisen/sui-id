// sui-id password-reset completion page (RFC 103 D10).
//
// The reset link is `/reset-password#t=<token>`. A URL fragment is never
// sent to the server, so the token reaches no proxy access log, trace span
// or server log. This script moves it into the form field that is POSTed,
// hides that field, and removes the fragment from the address bar and the
// current history entry. Without JavaScript the field stays visible and
// the user pastes the reset code from the email.
(function () {
  "use strict";
  var match = /(?:^#|&)t=([^&]+)/.exec(window.location.hash || "");
  if (!match) {
    return;
  }
  var token = decodeURIComponent(match[1]);
  var input = document.getElementById("reset-token");
  var field = document.getElementById("reset-token-field");
  if (input) {
    input.value = token;
  }
  if (field) {
    field.hidden = true;
  }
  if (window.history && window.history.replaceState) {
    window.history.replaceState(null, "", window.location.pathname + window.location.search);
  }
})();

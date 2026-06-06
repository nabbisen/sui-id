// sui-id logout CSRF token injector (v0.48.1 hotfix).
//
// The sign-out form in the admin nav has a hidden `_csrf` input
// whose value is not server-rendered (Shell does not have access
// to the per-request CSRF cookie value at the layout layer). This
// script reads `sui_id_csrf` from `document.cookie` and populates
// the input before submit.
//
// Loaded via `<script src="/static/logout-csrf.js" defer></script>`.
// Externalised in v0.48.1 to satisfy CSP `script-src 'self'` —
// the inline version it replaces was blocked, causing CSRF
// validation to fail on POST /admin/logout and the sign-out flow
// to redirect through /admin/login back to /admin (still
// authenticated), giving the appearance of a broken Sign Out
// button.
//
// Future work (v0.48.2 or later): server-render the CSRF token
// into the Shell so this script becomes unnecessary.

(function () {
  function inject() {
    var f = document.getElementById('logout-csrf');
    if (f && !f.value) {
      var m = document.cookie.match(/(?:^|; )sui_id_csrf=([^;]*)/);
      if (m) f.value = decodeURIComponent(m[1]);
    }
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', inject);
  } else {
    inject();
  }
})();

// sui-id theme initialization (RFC 062 + v0.48.1 hotfix + RFC 092).
//
// RFC 092 additions:
//   1. Swap the `<html class="no-js">` root class to `js` immediately so
//      CSS rules `.no-js .theme-toggle { display:none }` and
//      `.js .theme-no-js-note { display:none }` take effect before first paint.
//   2. localStorage access is already wrapped in try/catch; all storage
//      errors are swallowed and the CSS / system-preference fallback applies.
//
// Loaded as a BLOCKING script in <head> — no defer, no async.
// This is intentional: the root-class swap and data-theme attribute MUST
// be applied before the first style-dependent paint to prevent a flash of
// unstyled or wrong-theme content.

(function () {
  try {
    // RFC 092: swap no-js → js immediately. CSS uses this class to hide the
    // non-functional toggle button in no-JS environments.
    var root = document.documentElement;
    root.classList.replace('no-js', 'js');

    var KEY = "sui_id_theme";
    var saved;
    try { saved = localStorage.getItem(KEY); } catch (e) { saved = null; }
    var mode = (saved === "light" || saved === "dark") ? saved : "system";

    function apply(m) {
      if (m === "system") {
        root.removeAttribute("data-theme");
      } else {
        root.setAttribute("data-theme", m);
      }
    }

    apply(mode);

    if (mode === "system" && window.matchMedia) {
      var mq = window.matchMedia("(prefers-color-scheme: dark)");
      var listener = function () { /* CSS handles it via :not([data-theme]) */ };
      if (mq.addEventListener) mq.addEventListener("change", listener);
    }

    // Expose a tiny helper so the toggle buttons can also write to
    // localStorage and update `data-theme` immediately.
    window.__suiIdSetTheme = function (m) {
      if (m !== "light" && m !== "dark" && m !== "system") return;
      try { localStorage.setItem(KEY, m); } catch (e) {}
      apply(m);
      document.querySelectorAll(".theme-toggle__btn").forEach(function (b) {
        b.setAttribute(
          "aria-pressed",
          b.getAttribute("data-theme-value") === m ? "true" : "false"
        );
      });
    };

    // Attach click listeners to toggle buttons by `data-theme-value`.
    // Replaces inline `onclick=` handlers (blocked by CSP `script-src 'self'`).
    function attachToggleListeners() {
      document.querySelectorAll(".theme-toggle__btn[data-theme-value]")
        .forEach(function (b) {
          b.setAttribute(
            "aria-pressed",
            b.getAttribute("data-theme-value") === mode ? "true" : "false"
          );
          b.addEventListener("click", function () {
            window.__suiIdSetTheme(b.getAttribute("data-theme-value"));
          });
        });
    }

    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", attachToggleListeners);
    } else {
      attachToggleListeners();
    }
  } catch (e) {}
})();

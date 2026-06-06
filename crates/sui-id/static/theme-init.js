// sui-id theme initialization (RFC 062 + v0.48.1 hotfix).
//
// Runs early to apply the user's theme choice from localStorage
// before first paint, then attaches click listeners to the footer's
// theme toggle buttons (replacing v0.47.x inline `onclick=` handlers
// that were blocked by the CSP `script-src 'self'` policy).
//
// Loaded via `<script src="/static/theme-init.js" defer></script>`.
// The `defer` is fine here — FOUT protection is already provided by
// the synchronous CSS in <style>, and the theme application only
// affects the `data-theme` attribute on <html> which the CSS
// references through attribute selectors.

(function () {
  try {
    var KEY = "sui_id_theme";
    var saved = localStorage.getItem(KEY);
    var mode = (saved === "light" || saved === "dark") ? saved : "system";
    var root = document.documentElement;

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
    // Replaces inline `onclick=` handlers (blocked by CSP
    // `script-src 'self'` — `script-src-attr`).
    function attachToggleListeners() {
      document.querySelectorAll(".theme-toggle__btn[data-theme-value]")
        .forEach(function (b) {
          // aria-pressed initial state
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

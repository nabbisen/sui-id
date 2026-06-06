// sui-id clipboard helper (RFC 028 + v0.48.1 hotfix).
//
// Delegated `click` handler for any element carrying
// `data-copy="VALUE"`. Shows a localised confirmation label
// (from `data-copy-done`) for 1.8s then restores the original.
//
// Loaded via `<script src="/static/copy.js" defer></script>`.
// Previously inlined; externalised in v0.48.1 to satisfy the
// `Content-Security-Policy: script-src 'self'` rule that blocks
// inline `<script>` blocks.

(function () {
  if (!navigator.clipboard) return;
  // Mark document so CSS can show .copy-btn elements.
  document.documentElement.classList.add('clipboard-available');
  document.addEventListener('click', function (e) {
    var btn = e.target.closest('[data-copy]');
    if (!btn) return;
    var value = btn.getAttribute('data-copy');
    navigator.clipboard.writeText(value).then(function () {
      var orig = btn.textContent;
      btn.setAttribute('aria-pressed', 'true');
      // RFC 053: localised "Copied" text carried on each button via
      // data-copy-done. Falls back to a Unicode check + English if
      // missing.
      var done = btn.getAttribute('data-copy-done') || '\u2713 Copied';
      btn.textContent = done;
      setTimeout(function () {
        btn.textContent = orig;
        btn.removeAttribute('aria-pressed');
      }, 1800);
    }).catch(function () {});
  });
})();

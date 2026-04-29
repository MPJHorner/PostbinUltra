// Copy-to-clipboard for code blocks.
(function () {
  'use strict';
  document.addEventListener('click', function (e) {
    var btn = e.target.closest('.copy-btn');
    if (!btn) return;
    var text;
    // Buttons can either declare the exact text via data-clipboard (for
    // single-line CTAs like the install one-liner), or live next to a
    // `<pre><code>` block (the default for embedded code blocks).
    if (btn.dataset && btn.dataset.clipboard) {
      text = btn.dataset.clipboard;
    } else {
      var pre = btn.parentElement.querySelector('pre code');
      if (!pre) return;
      text = pre.innerText;
    }
    var done = function () {
      var prev = btn.textContent;
      btn.textContent = 'copied';
      btn.classList.add('copied');
      setTimeout(function () {
        btn.textContent = prev;
        btn.classList.remove('copied');
      }, 1400);
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, function () {
        legacyCopy(text);
        done();
      });
    } else {
      legacyCopy(text);
      done();
    }
  });

  function legacyCopy(text) {
    var ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand('copy'); } catch (e) {}
    document.body.removeChild(ta);
  }
})();

// Theme toggle + mobile nav. Mirrors the product app's pbu-theme localStorage key.
(function () {
  'use strict';

  var html = document.documentElement;
  var toggle = document.getElementById('theme-toggle');
  var navToggle = document.getElementById('nav-toggle');
  var navList = document.getElementById('nav-list');

  function setTheme(t) {
    html.dataset.theme = t;
    try { localStorage.setItem('pbu-theme', t); } catch (e) {}
  }

  if (!html.dataset.theme) {
    var prefers = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    html.dataset.theme = prefers;
  }

  if (toggle) {
    toggle.addEventListener('click', function () {
      setTheme(html.dataset.theme === 'light' ? 'dark' : 'light');
    });
  }

  if (navToggle && navList) {
    navToggle.addEventListener('click', function () {
      var open = navList.classList.toggle('open');
      navToggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
    navList.addEventListener('click', function (e) {
      if (e.target.closest('a')) {
        navList.classList.remove('open');
        navToggle.setAttribute('aria-expanded', 'false');
      }
    });
  }
})();

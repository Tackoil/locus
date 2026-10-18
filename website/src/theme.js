(() => {
  const root = document.documentElement;
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  let preference = 'system';

  try {
    const saved = localStorage.getItem('locus-theme');
    if (saved === 'light' || saved === 'dark') preference = saved;
  } catch {
    // The toggle still works when the browser disables persistent storage.
  }

  function applyTheme() {
    const dark = preference === 'dark' || (preference === 'system' && system.matches);
    root.dataset.theme = dark ? 'dark' : 'light';
    document.querySelector('meta[name="theme-color"]').setAttribute('content', dark ? '#151515' : '#fafafa');
    const toggle = document.getElementById('theme-toggle');
    if (toggle) {
      toggle.setAttribute('aria-pressed', String(dark));
      toggle.title = dark ? 'Switch to light theme' : 'Switch to dark theme';
    }
  }

  function toggleTheme() {
    preference = root.dataset.theme === 'dark' ? 'light' : 'dark';
    try {
      localStorage.setItem('locus-theme', preference);
    } catch {
      // Keep the selected appearance for this page without requiring storage.
    }
    applyTheme();
  }

  applyTheme();
  system.addEventListener('change', () => {
    if (preference === 'system') applyTheme();
  });
  document.addEventListener('DOMContentLoaded', () => {
    const toggle = document.getElementById('theme-toggle');
    toggle.addEventListener('click', toggleTheme);
    toggle.hidden = false;
    applyTheme();
  });
})();

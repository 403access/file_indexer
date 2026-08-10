/**
 * Theme manager — light / dark / system
 * Persists choice in localStorage and applies data-theme on <html>.
 */
(function () {
  const STORAGE_KEY = 'fi-theme';
  const THEMES = ['light', 'dark', 'system'];

  function getStored() {
    try {
      const v = localStorage.getItem(STORAGE_KEY);
      return THEMES.includes(v) ? v : 'system';
    } catch {
      return 'system';
    }
  }

  function systemPrefersDark() {
    return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
  }

  function resolvedTheme(preference) {
    if (preference === 'system') {
      return systemPrefersDark() ? 'dark' : 'light';
    }
    return preference;
  }

  function applyTheme(preference) {
    const pref = THEMES.includes(preference) ? preference : 'system';
    const resolved = resolvedTheme(pref);
    document.documentElement.setAttribute('data-theme', resolved);
    document.documentElement.setAttribute('data-theme-pref', pref);
    try {
      localStorage.setItem(STORAGE_KEY, pref);
    } catch {
      /* ignore */
    }
    updateSwitcherUI(pref);
    document.dispatchEvent(
      new CustomEvent('themechange', { detail: { preference: pref, resolved } })
    );
  }

  function updateSwitcherUI(preference) {
    document.querySelectorAll('[data-theme-option]').forEach((btn) => {
      const isActive = btn.getAttribute('data-theme-option') === preference;
      btn.classList.toggle('active', isActive);
      btn.setAttribute('aria-pressed', isActive ? 'true' : 'false');
    });
  }

  function cycleTheme() {
    const current = getStored();
    const idx = THEMES.indexOf(current);
    applyTheme(THEMES[(idx + 1) % THEMES.length]);
  }

  function setTheme(preference) {
    applyTheme(preference);
  }

  function getTheme() {
    return {
      preference: getStored(),
      resolved: resolvedTheme(getStored()),
    };
  }

  // Apply immediately (also run from inline head script for FOUC prevention)
  applyTheme(getStored());

  // React to OS changes when preference is "system"
  if (window.matchMedia) {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      if (getStored() === 'system') applyTheme('system');
    };
    if (mq.addEventListener) mq.addEventListener('change', onChange);
    else if (mq.addListener) mq.addListener(onChange);
  }

  // Delegate clicks on theme switcher buttons
  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-theme-option]');
    if (!btn) return;
    e.preventDefault();
    setTheme(btn.getAttribute('data-theme-option'));
  });

  // Re-sync switcher UI when DOM is ready (sidebar injects after)
  document.addEventListener('DOMContentLoaded', () => {
    updateSwitcherUI(getStored());
  });

  // Observe sidebar injection
  const observer = new MutationObserver(() => {
    if (document.querySelector('[data-theme-option]')) {
      updateSwitcherUI(getStored());
    }
  });
  if (document.documentElement) {
    observer.observe(document.documentElement, { childList: true, subtree: true });
  }

  window.Theme = { set: setTheme, get: getTheme, cycle: cycleTheme, apply: applyTheme };
})();

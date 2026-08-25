/**
 * Shared size formatting with user-selectable unit.
 * Preference: localStorage `fi-size-unit` = auto | B | KB | MB | GB | TB
 * Default: auto (largest unit that keeps the value >= 1).
 */
(function () {
  const STORAGE_KEY = 'fi-size-unit';
  const UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const PREFS = ['auto', 'B', 'KB', 'MB', 'GB', 'TB', 'PB'];

  function getStored() {
    try {
      const v = localStorage.getItem(STORAGE_KEY);
      return PREFS.includes(v) ? v : 'auto';
    } catch {
      return 'auto';
    }
  }

  function setStored(unit) {
    const pref = PREFS.includes(unit) ? unit : 'auto';
    try {
      localStorage.setItem(STORAGE_KEY, pref);
    } catch {
      /* ignore */
    }
    document.dispatchEvent(
      new CustomEvent('sizeunitchange', { detail: { unit: pref } })
    );
    return pref;
  }

  function formatSize(bytes, unitPref) {
    const n = Number(bytes);
    if (!Number.isFinite(n) || n < 0) return '—';
    if (n === 0) return '0 B';

    const pref = unitPref || getStored();
    const base = 1024;

    if (pref === 'auto') {
      let i = Math.floor(Math.log(n) / Math.log(base));
      i = Math.max(0, Math.min(i, UNITS.length - 1));
      const value = n / Math.pow(base, i);
      const text =
        i === 0
          ? String(Math.round(value))
          : value.toFixed(2).replace(/\.?0+$/, '');
      return `${text} ${UNITS[i]}`;
    }

    const idx = UNITS.indexOf(pref);
    if (idx < 0) return formatSize(n, 'auto');
    const value = n / Math.pow(base, idx);
    const text =
      idx === 0
        ? String(Math.round(value))
        : value >= 100
          ? value.toFixed(1).replace(/\.0$/, '')
          : value.toFixed(2).replace(/\.?0+$/, '');
    return `${text} ${UNITS[idx]}`;
  }

  function updateSwitcherUI(unit) {
    const current = unit || getStored();
    document.querySelectorAll('[data-size-unit-option]').forEach((btn) => {
      const isActive = btn.getAttribute('data-size-unit-option') === current;
      btn.classList.toggle('active', isActive);
      btn.setAttribute('aria-pressed', isActive ? 'true' : 'false');
    });
    document.querySelectorAll('[data-size-unit-select]').forEach((sel) => {
      if (sel.value !== current) sel.value = current;
    });
  }

  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-size-unit-option]');
    if (!btn) return;
    e.preventDefault();
    const unit = setStored(btn.getAttribute('data-size-unit-option'));
    updateSwitcherUI(unit);
  });

  document.addEventListener('change', (e) => {
    const sel = e.target.closest('[data-size-unit-select]');
    if (!sel) return;
    const unit = setStored(sel.value);
    updateSwitcherUI(unit);
  });

  document.addEventListener('DOMContentLoaded', () => {
    updateSwitcherUI(getStored());
  });

  window.SizeFormat = {
    get: getStored,
    set: setStored,
    format: formatSize,
    units: PREFS.slice(),
    updateUI: updateSwitcherUI,
  };

  window.formatSize = function formatSizeGlobal(bytes) {
    return formatSize(bytes);
  };
})();

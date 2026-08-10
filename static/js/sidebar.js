/**
 * App navigation — single source of truth for menu structure.
 * Supports sidebar and topbar layouts (preference: localStorage fi-nav-layout).
 * Injects into #sidebar-container on every page.
 */

const SIDEBAR_ICONS = {
  dashboard: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/></svg>`,
  search: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>`,
  explorer: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></svg>`,
  duplicates: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><rect x="8" y="8" width="12" height="12" rx="1.5"/><path d="M4 16V6a2 2 0 0 1 2-2h10"/></svg>`,
  folders: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h3.5l1.5 2H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/><path d="M3 11h18"/></svg>`,
  skipped: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="m9 9 6 6M15 9l-6 6"/></svg>`,
  ignored: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3 4 7v5c0 5 3.5 8.5 8 9 4.5-.5 8-4 8-9V7l-8-4z"/><path d="M9.5 12.5 11 14l3.5-3.5"/></svg>`,
  status: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path d="M22 12A10 10 0 1 1 12 2"/><path d="M22 2v6h-6"/><path d="M12 6v6l4 2"/></svg>`,
  processes: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h16M4 12h10M4 18h14"/><circle cx="18" cy="12" r="2"/><circle cx="20" cy="18" r="2"/></svg>`,
  logs: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>`,
  brand: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/></svg>`,
  menu: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 7h16M4 12h16M4 17h16"/></svg>`,
  sun: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"/></svg>`,
  moon: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 14.5A8.5 8.5 0 1 1 9.5 3a7 7 0 0 0 11.5 11.5z"/></svg>`,
  system: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>`,
};

const SIDEBAR_ITEMS = [
  {
    section: 'Browse',
    links: [
      { href: '/', label: 'Dashboard', icon: 'dashboard' },
      { href: '/search.html', label: 'Search', icon: 'search' },
      { href: '/explorer.html', label: 'Explorer', icon: 'explorer' },
    ],
  },
  {
    section: 'Duplicates',
    links: [
      { href: '/duplicates.html', label: 'Files', icon: 'duplicates' },
      { href: '/duplicate-folders.html', label: 'Folders', icon: 'folders' },
    ],
  },
  {
    section: 'Data',
    links: [
      { href: '/skipped.html', label: 'Skipped', icon: 'skipped' },
      { href: '/ignored.html', label: 'Ignored', icon: 'ignored' },
    ],
  },
  {
    section: 'System',
    links: [
      { href: '/status.html', label: 'Status', icon: 'status' },
      { href: '/processes.html', label: 'Processes', icon: 'processes' },
      { href: '/logs.html', label: 'Logs', icon: 'logs' },
      { href: '/settings.html', label: 'Settings', icon: 'settings' },
    ],
  },
];

const NAV_LAYOUT_KEY = 'fi-nav-layout';
const NAV_LAYOUTS = ['sidebar', 'topbar'];

function getNavLayout() {
  try {
    const v = localStorage.getItem(NAV_LAYOUT_KEY);
    return NAV_LAYOUTS.includes(v) ? v : 'sidebar';
  } catch {
    return 'sidebar';
  }
}

function applyNavLayout(layout) {
  const value = NAV_LAYOUTS.includes(layout) ? layout : 'sidebar';
  document.documentElement.setAttribute('data-nav-layout', value);
  try {
    localStorage.setItem(NAV_LAYOUT_KEY, value);
  } catch {
    /* ignore */
  }
  document.dispatchEvent(
    new CustomEvent('navlayoutchange', { detail: { layout: value } })
  );
  return value;
}

function setNavLayout(layout) {
  const value = applyNavLayout(layout);
  injectSidebar();
  updateNavLayoutSwitcherUI(value);
  return value;
}

function updateNavLayoutSwitcherUI(layout) {
  const current = layout || getNavLayout();
  document.querySelectorAll('[data-nav-layout-option]').forEach((btn) => {
    const isActive = btn.getAttribute('data-nav-layout-option') === current;
    btn.classList.toggle('active', isActive);
    btn.setAttribute('aria-pressed', isActive ? 'true' : 'false');
  });
}

function getActiveHref() {
  const path = window.location.pathname;
  if (path === '' || path === '/index.html') return '/';
  return path;
}

function isActiveLink(href, activeHref) {
  if (href === activeHref) return true;
  if (href === '/' && (activeHref === '/' || activeHref === '/index.html')) return true;
  return false;
}

function escapeSidebarHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function buildNavLink(link, activeHref, extraClass) {
  const active = isActiveLink(link.href, activeHref) ? ' active' : '';
  const icon = SIDEBAR_ICONS[link.icon] || '';
  const cls = extraClass || 'nav-item';
  return `          <a href="${link.href}" class="${cls}${active}"${active ? ' aria-current="page"' : ''}>
            <span class="nav-item__icon" aria-hidden="true">${icon}</span>
            <span class="nav-item__label">${link.label}</span>
          </a>`;
}

function buildThemeSwitcher() {
  return `<div class="theme-switcher" role="group" aria-label="Color theme">
          <button type="button" class="theme-switcher__btn" data-theme-option="light" title="Light" aria-label="Light theme">
            ${SIDEBAR_ICONS.sun}
          </button>
          <button type="button" class="theme-switcher__btn" data-theme-option="dark" title="Dark" aria-label="Dark theme">
            ${SIDEBAR_ICONS.moon}
          </button>
          <button type="button" class="theme-switcher__btn" data-theme-option="system" title="System" aria-label="System theme">
            ${SIDEBAR_ICONS.system}
          </button>
        </div>`;
}

function buildStatusBlock(textClass) {
  return `<div class="sidebar-footer__status">
            <span id="status-dot" class="status-dot idle" title="Idle"></span>
            <span class="${textClass || 'sidebar-footer-text'}">System idle</span>
          </div>`;
}

function buildSidebar() {
  const activeHref = getActiveHref();
  const pageTitle =
    document.title?.replace(/\s*[-–|].*$/, '').trim() || 'File Indexer';

  const sectionsHtml = SIDEBAR_ITEMS.map(({ section, links }) => {
    const linksHtml = links
      .map((link) => buildNavLink(link, activeHref))
      .join('\n');
    return `        <div class="nav-section">
          <div class="nav-section-title">${section}</div>
${linksHtml}
        </div>`;
  }).join('\n');

  return `
    <div class="mobile-bar">
      <button type="button" class="mobile-bar__btn" onclick="toggleSidebar()" aria-label="Open menu">
        ${SIDEBAR_ICONS.menu}
      </button>
      <span class="mobile-bar__title">${escapeSidebarHtml(pageTitle)}</span>
    </div>
    <aside class="sidebar" id="sidebar" aria-label="Main navigation">
      <div class="sidebar-header">
        <a href="/" class="sidebar-brand">
          <span class="sidebar-brand__mark" aria-hidden="true">${SIDEBAR_ICONS.brand}</span>
          <span class="sidebar-brand__text">
            <h1 class="sidebar-brand__title">File Indexer</h1>
            <span class="sidebar-brand__tag">Local library</span>
          </span>
        </a>
        <button type="button" class="sidebar-toggle" onclick="toggleSidebar()" aria-label="Close menu">
          ${SIDEBAR_ICONS.menu}
        </button>
      </div>
      <nav class="sidebar-nav">
${sectionsHtml}
      </nav>
      <div class="sidebar-footer">
        <div class="sidebar-footer__row">
          ${buildStatusBlock()}
        </div>
        ${buildThemeSwitcher()}
      </div>
    </aside>
    <div class="sidebar-overlay" id="sidebar-overlay" onclick="toggleSidebar()"></div>`;
}

function buildTopbar() {
  const activeHref = getActiveHref();
  const pageTitle =
    document.title?.replace(/\s*[-–|].*$/, '').trim() || 'File Indexer';

  const sectionsHtml = SIDEBAR_ITEMS.map(({ section, links }, idx) => {
    const linksHtml = links
      .map((link) => buildNavLink(link, activeHref, 'nav-item topbar-nav__item'))
      .join('\n');
    const divider =
      idx > 0 ? '<span class="topbar-nav__divider" aria-hidden="true"></span>' : '';
    return `${divider}
        <div class="topbar-nav__section" data-section="${escapeSidebarHtml(section)}">
          <span class="topbar-nav__section-label">${section}</span>
${linksHtml}
        </div>`;
  }).join('\n');

  // Mobile panel: full list with section titles (same structure as sidebar)
  const mobileSectionsHtml = SIDEBAR_ITEMS.map(({ section, links }) => {
    const linksHtml = links
      .map((link) => buildNavLink(link, activeHref))
      .join('\n');
    return `        <div class="nav-section">
          <div class="nav-section-title">${section}</div>
${linksHtml}
        </div>`;
  }).join('\n');

  return `
    <header class="topbar" id="topbar" aria-label="Main navigation">
      <div class="topbar__inner">
        <a href="/" class="topbar-brand sidebar-brand">
          <span class="sidebar-brand__mark" aria-hidden="true">${SIDEBAR_ICONS.brand}</span>
          <span class="sidebar-brand__text">
            <span class="sidebar-brand__title">File Indexer</span>
            <span class="sidebar-brand__tag">Local library</span>
          </span>
        </a>
        <nav class="topbar-nav" aria-label="Primary">
${sectionsHtml}
        </nav>
        <div class="topbar-actions">
          ${buildStatusBlock('topbar-status-text')}
          <div class="topbar-actions__theme">
            ${buildThemeSwitcher()}
          </div>
          <button type="button" class="topbar-toggle" onclick="toggleTopbarMenu()" aria-label="Open menu" aria-expanded="false" aria-controls="topbar-panel">
            ${SIDEBAR_ICONS.menu}
          </button>
        </div>
      </div>
      <div class="topbar-panel" id="topbar-panel">
        <div class="topbar-panel__header">
          <span class="topbar-panel__title">${escapeSidebarHtml(pageTitle)}</span>
          <button type="button" class="topbar-panel__close" onclick="toggleTopbarMenu()" aria-label="Close menu">
            ${SIDEBAR_ICONS.menu}
          </button>
        </div>
        <nav class="topbar-panel__nav">
${mobileSectionsHtml}
        </nav>
        <div class="topbar-panel__footer">
          ${buildThemeSwitcher()}
        </div>
      </div>
    </header>
    <div class="topbar-overlay" id="topbar-overlay" onclick="toggleTopbarMenu()"></div>`;
}

function wireStatusText(container) {
  const dot = document.getElementById('status-dot');
  if (!dot) return;
  const textEls = container.querySelectorAll('.sidebar-footer-text, .topbar-status-text');
  if (!textEls.length) return;
  const sync = () => {
    let label = 'System idle';
    if (dot.classList.contains('indexing')) label = 'Indexing…';
    else if (dot.classList.contains('error')) label = 'Unreachable';
    textEls.forEach((el) => {
      el.textContent = label;
    });
  };
  const obs = new MutationObserver(sync);
  obs.observe(dot, { attributes: true, attributeFilter: ['class', 'title'] });
  sync();
}

function injectSidebar() {
  const container = document.getElementById('sidebar-container');
  if (!container) return;

  const layout = getNavLayout();
  applyNavLayout(layout);
  document.body.style.overflow = '';

  container.innerHTML = layout === 'topbar' ? buildTopbar() : buildSidebar();
  wireStatusText(container);
  updateNavLayoutSwitcherUI(layout);

  // Keep theme switcher active state in sync after re-inject
  if (window.Theme && typeof window.Theme.get === 'function') {
    const pref = window.Theme.get().preference;
    document.querySelectorAll('[data-theme-option]').forEach((btn) => {
      const isActive = btn.getAttribute('data-theme-option') === pref;
      btn.classList.toggle('active', isActive);
      btn.setAttribute('aria-pressed', isActive ? 'true' : 'false');
    });
  }
}

function toggleSidebar() {
  const sidebar = document.getElementById('sidebar');
  const overlay = document.getElementById('sidebar-overlay');
  if (!sidebar) return;
  const isOpen = sidebar.classList.contains('open');
  sidebar.classList.toggle('open', !isOpen);
  if (overlay) overlay.classList.toggle('open', !isOpen);
  document.body.style.overflow = !isOpen ? 'hidden' : '';
}

function toggleTopbarMenu() {
  const panel = document.getElementById('topbar-panel');
  const overlay = document.getElementById('topbar-overlay');
  const toggle = document.querySelector('.topbar-toggle');
  if (!panel) return;
  const isOpen = panel.classList.contains('open');
  panel.classList.toggle('open', !isOpen);
  if (overlay) overlay.classList.toggle('open', !isOpen);
  if (toggle) toggle.setAttribute('aria-expanded', !isOpen ? 'true' : 'false');
  document.body.style.overflow = !isOpen ? 'hidden' : '';
}

function closeMobileNav() {
  const sidebar = document.getElementById('sidebar');
  if (sidebar?.classList.contains('open')) toggleSidebar();
  const panel = document.getElementById('topbar-panel');
  if (panel?.classList.contains('open')) toggleTopbarMenu();
}

// Apply layout attribute early (also set from inline head script)
applyNavLayout(getNavLayout());

document.addEventListener('DOMContentLoaded', () => {
  injectSidebar();
  updateNavLayoutSwitcherUI(getNavLayout());
});

// Close mobile nav on Escape
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  closeMobileNav();
});

// Delegate clicks on nav-layout switcher buttons
document.addEventListener('click', (e) => {
  const btn = e.target.closest('[data-nav-layout-option]');
  if (!btn) return;
  e.preventDefault();
  setNavLayout(btn.getAttribute('data-nav-layout-option'));
});

window.NavLayout = {
  get: getNavLayout,
  set: setNavLayout,
  apply: applyNavLayout,
  layouts: NAV_LAYOUTS.slice(),
};

// Keep legacy global for onclick handlers
window.toggleSidebar = toggleSidebar;
window.toggleTopbarMenu = toggleTopbarMenu;

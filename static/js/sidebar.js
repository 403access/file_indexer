const SIDEBAR_ITEMS = [
  {
    section: 'Browse',
    links: [
      { href: '/', label: 'Dashboard' },
      { href: '/search.html', label: 'Search' },
      { href: '/explorer.html', label: 'Explorer' },
    ],
  },
  {
    section: 'Duplicates',
    links: [
      { href: '/duplicates.html', label: 'Duplicates' },
      { href: '/duplicate-folders.html', label: 'Folders' },
    ],
  },
  {
    section: 'Data',
    links: [
      { href: '/skipped.html', label: 'Skipped' },
      { href: '/ignored.html', label: 'Ignored' },
    ],
  },
  {
    section: 'System',
    links: [
      { href: '/status.html', label: 'Status' },
      { href: '/processes.html', label: 'Processes' },
      { href: '/logs.html', label: 'Logs' },
    ],
  },
  {
    section: 'Config',
    links: [
      { href: '/settings.html', label: 'Settings' },
    ],
  },
];

function getActiveHref() {
  return window.location.pathname;
}

function buildSidebar() {
  const activeHref = getActiveHref();
  const sectionsHtml = SIDEBAR_ITEMS.map(({ section, links }) => {
    const linksHtml = links
      .map(
        (link) => {
          const isActive = link.href === activeHref ? ' active' : '';
          return `          <a href="${link.href}" class="nav-item${isActive}">${link.label}</a>`;
        }
      )
      .join('\n');
    return `        <div class="nav-section">
          <div class="nav-section-title">${section}</div>
${linksHtml}
        </div>`;
  }).join('\n');

  return `    <aside class="sidebar" id="sidebar">
      <div class="sidebar-header">
        <h1>File Indexer</h1>
        <button class="sidebar-toggle" onclick="toggleSidebar()" aria-label="Toggle menu">☰</button>
      </div>
      <nav class="sidebar-nav">
${sectionsHtml}
      </nav>
      <div class="sidebar-footer">
        <span class="sidebar-footer-text">Indexer</span>
        <span id="status-dot" class="status-dot idle" title="Idle"></span>
      </div>
    </aside>
    <div class="sidebar-overlay" id="sidebar-overlay" onclick="toggleSidebar()"></div>`;
}

function injectSidebar() {
  const container = document.getElementById('sidebar-container');
  if (!container) return;
  container.innerHTML = buildSidebar();
}

function toggleSidebar() {
  const sidebar = document.getElementById('sidebar');
  const overlay = document.getElementById('sidebar-overlay');
  if (!sidebar) return;
  const isOpen = sidebar.classList.contains('open');
  if (isOpen) {
    sidebar.classList.remove('open');
    if (overlay) overlay.classList.remove('open');
  } else {
    sidebar.classList.add('open');
    if (overlay) overlay.classList.add('open');
  }
}

document.addEventListener('DOMContentLoaded', injectSidebar);

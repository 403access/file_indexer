/**
 * Reusable right-side Drawer component.
 *
 * Usage:
 *   const d = Drawer.create({
 *     id: 'my-drawer',
 *     title: 'Details',
 *     size: 'md' | 'lg' | 'xl',   // optional
 *     onClose: () => {},         // optional
 *   });
 *   d.setTitle('Process #12');
 *   d.setBody(htmlOrNode);
 *   d.setMeta(htmlOrNode);       // optional meta strip under header
 *   d.open();
 *   d.close();
 *   d.destroy();
 */
(function () {
  const SIZE_CLASS = {
    md: '',
    lg: 'drawer--lg',
    xl: 'drawer--xl',
  };

  let openCount = 0;

  function lockScroll(lock) {
    document.body.style.overflow = lock ? 'hidden' : '';
  }

  function create(options = {}) {
    const id = options.id || `drawer-${Math.random().toString(36).slice(2, 9)}`;
    const size = options.size || 'md';
    const sizeClass = SIZE_CLASS[size] || '';

    // Reuse existing DOM if present
    let overlay = document.getElementById(`${id}-overlay`);
    let panel = document.getElementById(id);

    if (!overlay) {
      overlay = document.createElement('div');
      overlay.className = 'drawer-overlay';
      overlay.id = `${id}-overlay`;
      overlay.addEventListener('click', () => api.close());
      document.body.appendChild(overlay);
    }

    if (!panel) {
      panel = document.createElement('aside');
      panel.className = `drawer ${sizeClass}`.trim();
      panel.id = id;
      panel.setAttribute('role', 'dialog');
      panel.setAttribute('aria-modal', 'true');
      panel.innerHTML = `
        <div class="drawer__header">
          <h3 class="drawer__title" data-drawer-title></h3>
          <button type="button" class="drawer__close" data-drawer-close aria-label="Close">&times;</button>
        </div>
        <div class="drawer__meta" data-drawer-meta style="display:none"></div>
        <div class="drawer__body" data-drawer-body></div>
        <div class="drawer__footer" data-drawer-footer style="display:none"></div>
      `;
      document.body.appendChild(panel);
    } else if (sizeClass && !panel.classList.contains(sizeClass)) {
      panel.classList.remove('drawer--lg', 'drawer--xl');
      if (sizeClass) panel.classList.add(sizeClass);
    }

    const titleEl = panel.querySelector('[data-drawer-title]');
    const metaEl = panel.querySelector('[data-drawer-meta]');
    const bodyEl = panel.querySelector('[data-drawer-body]');
    const footerEl = panel.querySelector('[data-drawer-footer]');
    const closeBtn = panel.querySelector('[data-drawer-close]');

    if (options.title) titleEl.textContent = options.title;
    closeBtn.onclick = () => api.close();

    function onKey(e) {
      if (e.key === 'Escape') api.close();
    }

    const api = {
      el: panel,
      overlay,
      bodyEl,
      metaEl,
      footerEl,
      titleEl,

      setTitle(text) {
        titleEl.textContent = text || '';
        return api;
      },

      setBody(content) {
        if (content == null) {
          bodyEl.innerHTML = '';
        } else if (typeof content === 'string') {
          bodyEl.innerHTML = content;
        } else {
          bodyEl.innerHTML = '';
          bodyEl.appendChild(content);
        }
        return api;
      },

      setMeta(content) {
        if (content == null || content === '') {
          metaEl.style.display = 'none';
          metaEl.innerHTML = '';
        } else {
          metaEl.style.display = '';
          if (typeof content === 'string') metaEl.innerHTML = content;
          else {
            metaEl.innerHTML = '';
            metaEl.appendChild(content);
          }
        }
        return api;
      },

      setFooter(content) {
        if (content == null || content === '') {
          footerEl.style.display = 'none';
          footerEl.innerHTML = '';
        } else {
          footerEl.style.display = 'flex';
          if (typeof content === 'string') footerEl.innerHTML = content;
          else {
            footerEl.innerHTML = '';
            footerEl.appendChild(content);
          }
        }
        return api;
      },

      open() {
        if (!panel.classList.contains('open')) {
          openCount += 1;
          lockScroll(true);
        }
        // force reflow then animate
        void panel.offsetWidth;
        overlay.classList.add('open');
        panel.classList.add('open');
        document.addEventListener('keydown', onKey);
        if (typeof options.onOpen === 'function') options.onOpen(api);
        return api;
      },

      close() {
        if (panel.classList.contains('open')) {
          openCount = Math.max(0, openCount - 1);
          if (openCount === 0) lockScroll(false);
        }
        overlay.classList.remove('open');
        panel.classList.remove('open');
        document.removeEventListener('keydown', onKey);
        if (typeof options.onClose === 'function') options.onClose(api);
        return api;
      },

      isOpen() {
        return panel.classList.contains('open');
      },

      destroy() {
        api.close();
        overlay.remove();
        panel.remove();
      },
    };

    return api;
  }

  /** Open a one-off drawer with content (convenience). */
  function open(options) {
    const d = create(options);
    if (options.body != null) d.setBody(options.body);
    if (options.meta != null) d.setMeta(options.meta);
    if (options.footer != null) d.setFooter(options.footer);
    d.open();
    return d;
  }

  window.Drawer = { create, open };
})();

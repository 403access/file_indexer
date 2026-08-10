# Web UI & Design System

The File Indexer ships a static web UI served by the Axum server from the `static/` directory. This document describes the design system, theme switching, reusable components, page structure, and how to extend the UI consistently.

## Overview

| Concern | Location |
|---|---|
| Design tokens (colors, spacing, typography) | `static/css/tokens.css` |
| Reusable UI components | `static/css/components.css` |
| App shell (sidebar layout, base) | `static/css/style.css` |
| Page-specific styles | `static/css/processes.css`, `file-viewer.css`, `duplicate-folders.css` |
| Theme manager | `static/js/theme.js` |
| Navigation sidebar | `static/js/sidebar.js` |
| Right-hand drawer | `static/js/drawer.js` |
| Shared helpers / folder drawer | `static/js/app.js` |
| Pages | `static/*.html` |

There is **no frontend build step**. HTML, CSS, and JS are plain static assets. Prefer CSS variables and shared component classes over one-off colors.

---

## Architecture

```
static/
├── css/
│   ├── tokens.css          # Light/dark design tokens
│   ├── components.css      # Buttons, cards, tables, drawers, …
│   ├── style.css           # App shell + sidebar layout
│   ├── processes.css       # Processes page
│   ├── file-viewer.css     # File content viewer panel
│   └── duplicate-folders.css
├── js/
│   ├── theme.js            # Theme API + persistence
│   ├── sidebar.js          # Nav injection (single source of truth)
│   ├── drawer.js           # Reusable Drawer component
│   ├── app.js              # API helpers, folder drawer, formatters
│   ├── status.js           # Sidebar status-dot polling
│   └── …                   # Page-specific modules
└── *.html                  # One page per route
```

### Stylesheet load order

Every page should link stylesheets in this order:

```html
<link rel="stylesheet" href="/css/tokens.css">
<link rel="stylesheet" href="/css/components.css">
<link rel="stylesheet" href="/css/style.css">
<!-- optional page-specific CSS -->
```

### Script load order (typical)

```html
<script src="/js/theme.js"></script>
<script src="/js/drawer.js"></script>
<script src="/js/sidebar.js"></script>
<script src="/js/status.js"></script>
<!-- page-specific scripts (app.js, search.js, …) -->
```

---

## Themes

### Modes

| Preference | Behavior |
|---|---|
| `light` | Force light palette |
| `dark` | Force dark palette |
| `system` (default) | Follow `prefers-color-scheme` |

### How it works

1. **FOUC prevention** — a tiny inline script in `<head>` reads `localStorage` key `fi-theme` and sets `data-theme` on `<html>` before paint.
2. **`theme.js`** re-applies the preference, listens for OS theme changes when preference is `system`, and updates switcher buttons.
3. Tokens in `tokens.css` are scoped with:
   - `:root` / `[data-theme="light"]` — light values
   - `[data-theme="dark"]` — dark values
   - `@media (prefers-color-scheme: dark)` fallback when no explicit theme is set

### Attributes on `<html>`

| Attribute | Meaning |
|---|---|
| `data-theme` | Resolved theme actually applied (`light` or `dark`) |
| `data-theme-pref` | User preference (`light`, `dark`, or `system`) |

### Theme API (`window.Theme`)

```js
Theme.set('dark');           // set preference
Theme.get();                 // { preference, resolved }
Theme.cycle();               // light → dark → system → …
Theme.apply('system');       // same as set
```

### Theme change event

```js
document.addEventListener('themechange', (e) => {
  const { preference, resolved } = e.detail;
  // e.g. re-render charts with new colors
});
```

### UI control

The sidebar footer includes a three-button **theme switcher** (sun / moon / monitor). Buttons use `data-theme-option="light|dark|system"`. Clicks are delegated in `theme.js`.

### Storage

| Key | Values |
|---|---|
| `localStorage['fi-theme']` | `light` \| `dark` \| `system` |

---

## Design tokens

All visual styling should use CSS custom properties from `tokens.css`. Do **not** hardcode hex colors in page CSS or inline styles.

### Surfaces

| Token | Use |
|---|---|
| `--bg-app` | Page background |
| `--bg-elevated` | Cards, panels, tables |
| `--bg-muted` | Subtle strips, headers |
| `--bg-hover` / `--bg-active` | Interactive hover/active |
| `--bg-overlay` | Modal/drawer scrim |
| `--bg-sidebar` | Nav sidebar background |

### Text

| Token | Use |
|---|---|
| `--text-primary` | Headings, body |
| `--text-secondary` | Supporting text |
| `--text-muted` | Labels, hints |
| `--text-link` | Links |

### Accents & semantic

| Token | Use |
|---|---|
| `--accent`, `--accent-hover`, `--accent-muted` | Primary actions, focus |
| `--success` / `--warning` / `--danger` / `--info` | Status colors |
| Matching `*-muted` and `*-text` | Badges and soft backgrounds |

### Layout & type

| Token | Default (approx.) |
|---|---|
| `--sidebar-width` | `260px` |
| `--drawer-width` | `460px` |
| `--main-max` | `1280px` |
| `--font-sans` | Inter + system stack |
| `--font-mono` | JetBrains Mono + system mono |
| `--radius-sm` / `--radius-md` / `--radius-lg` | Control / card radii |
| `--space-1` … `--space-8` | Spacing scale |
| `--shadow-xs` … `--shadow-lg` | Elevation |

When adding a new color, define it in **both** light and dark sections of `tokens.css`.

---

## Reusable components

Defined in `static/css/components.css`. Prefer these classes over custom one-offs.

### Buttons

```html
<button type="button" class="btn">Primary</button>
<button type="button" class="btn btn--secondary btn--sm">Secondary</button>
<button type="button" class="btn btn--ghost">Ghost</button>
<button type="button" class="btn btn--danger">Delete</button>
<button type="button" class="btn btn--success btn--sm">Save</button>
```

| Class | Role |
|---|---|
| `.btn` | Base primary button |
| `.btn--sm` / `.btn--lg` | Sizes |
| `.btn--secondary` | Outlined elevated |
| `.btn--ghost` | Transparent |
| `.btn--danger` / `.btn--success` / `.btn--warning` | Semantic |
| `.btn--outline` | Accent outline |
| `.btn--icon` | Square icon-only |

### Cards

```html
<div class="card">
  <div class="card__header">
    <h2 class="card__title">Title</h2>
  </div>
  <div class="card__body">…</div>
  <div class="card__footer">…</div>
</div>
```

### Stat cards

```html
<section class="stat-grid">
  <div class="stat-card">
    <div class="stat-card__value">1,234</div>
    <div class="stat-card__label">Files</div>
  </div>
  <div class="stat-card stat-card--danger">…</div>
</section>
```

Modifiers: `--danger`, `--warning`, `--purple`, `--success`, `--accent`, `--clickable`.

### Page header

```html
<header class="page-header">
  <div class="page-header__titles">
    <h1 class="page-header__title">Search</h1>
    <p class="page-header__subtitle">Find files and folders</p>
  </div>
  <div class="page-header__actions">
    <button type="button" class="btn btn--secondary btn--sm">Refresh</button>
  </div>
</header>
```

### Forms & toolbars

- `.toolbar` / `.search-controls` — filter bars
- `.form-row`, `.form-group`, `.form-label`, `.form-hint`
- Native `input`, `select`, `textarea` pick up shared control styles

### Segmented control

```html
<div class="segmented" role="group" aria-label="Interval">
  <button type="button" class="segmented__btn active">Year</button>
  <button type="button" class="segmented__btn">Month</button>
</div>
```

### Tables

```html
<div class="table-wrap">
  <table class="table">
    <thead>…</thead>
    <tbody>…</tbody>
  </table>
</div>
```

Cell helpers: `.name`, `.path`, `.size`, `.type`, `.mono`.

### Badges & status

```html
<span class="badge badge--accent">Label</span>
<span class="status-badge active">active</span>
<span class="category-badge">indexing</span>
```

Status classes: `.active`, `.completed`, `.failed`, `.pending`.

### Progress

```html
<div class="progress">
  <div class="progress__bar" style="width: 40%"></div>
</div>
```

Legacy aliases used by processes: `.progress-bar-bg`, `.progress-bar-fill`.

### Empty states

```html
<div class="empty-state">
  <div class="empty-state__title">No results</div>
  <p class="empty-state__desc">Try a different query</p>
</div>
```

### Toggle

```html
<label class="toggle">
  <input type="checkbox">
  <span class="toggle-slider"></span>
  <span class="toggle-text">Enabled</span>
</label>
```

### Modal

```html
<div class="modal-overlay open">
  <div class="modal">
    <div class="modal__header">
      <h2 class="modal__title">Title</h2>
      <button type="button" class="drawer__close">×</button>
    </div>
    <div class="modal__body">…</div>
    <div class="modal__footer">…</div>
  </div>
</div>
```

### Info bar

```html
<div class="info-bar">
  <span>Last snapshot: <strong>—</strong></span>
</div>
```

---

## App shell: sidebar

### Injection

Every page includes:

```html
<nav id="sidebar-container"></nav>
```

`sidebar.js` replaces that container on `DOMContentLoaded` with the full nav, mobile bar, overlay, status dot, and theme switcher.

### Navigation structure

Defined once in `SIDEBAR_ITEMS` inside `static/js/sidebar.js`:

| Section | Links |
|---|---|
| Browse | Dashboard, Search, Explorer |
| Duplicates | Files, Folders |
| Data | Skipped, Ignored |
| System | Status, Processes, Logs, Settings |

To add a page: create the HTML file, add an entry to `SIDEBAR_ITEMS` (with `href`, `label`, `icon`), and optionally add an SVG to `SIDEBAR_ICONS`.

### Mobile

Below `768px` the sidebar slides in/out; a sticky mobile bar with a menu button is shown. Overlay and Escape close the menu.

### Status dot

`#status-dot` is updated by `status.js` (polls `/api/status`):

| Class | Meaning |
|---|---|
| `.idle` | Server reachable, not indexing |
| `.indexing` | Indexing in progress |
| `.error` | Unreachable |

---

## Drawer component

Right-side slide-over panel for details (folders, processes, ignore rules, etc.).

### API (`window.Drawer`)

```js
const d = Drawer.create({
  id: 'my-drawer',       // stable DOM id (reused across opens)
  title: 'Details',
  size: 'md',            // 'md' | 'lg' | 'xl'
  onOpen: (api) => {},
  onClose: (api) => {},
});

d.setTitle('Folder name');
d.setMeta('<div class="meta-row">…</div>');  // strip under header; '' hides
d.setBody(htmlOrNode);
d.setFooter(htmlOrNode);                     // '' hides
d.open();
d.close();
d.isOpen();
d.destroy();
```

Convenience:

```js
Drawer.open({ id: 'once', title: '…', body: '…', size: 'lg' });
```

### Markup classes (CSS)

| Class | Role |
|---|---|
| `.drawer-overlay` | Scrim |
| `.drawer` / `.drawer--lg` / `.drawer--xl` | Panel |
| `.drawer__header` / `__title` / `__close` | Header |
| `.drawer__meta` | Optional meta strip |
| `.drawer__body` / `__footer` | Content |
| `.drawer__section` / `__section-title` | Sections |
| `.meta-row` / `__label` / `__value` | Key/value rows |
| `.list-row` / `__icon` / `__name` / `__meta` | Clickable lists |

### Current usages

| Feature | Implementation |
|---|---|
| Folder details (search results) | `app.js` → `Drawer` (`folder-drawer`) |
| Ignore rule details | `ignored.html` → `Drawer` (`ignore-rule-drawer`) |
| Process details | `processes.html` + `processes.js` (dedicated markup, same visual tokens) |
| File viewer | `file-viewer.js` (dedicated panel, themed via `file-viewer.css`) |

Prefer **`Drawer.create`** for new detail panels so behavior (Escape, scroll lock, overlay) stays consistent.

---

## Page checklist (new page)

1. Create `static/your-page.html`.
2. Copy the standard head bootstrap:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <script>
    (function () {
      try {
        var t = localStorage.getItem('fi-theme') || 'system';
        var d = t === 'system'
          ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
          : t;
        document.documentElement.setAttribute('data-theme', d);
        document.documentElement.setAttribute('data-theme-pref', t);
      } catch (e) {
        document.documentElement.setAttribute('data-theme', 'dark');
      }
    })();
  </script>
  <title>Your Page - File Indexer</title>
  <link rel="stylesheet" href="/css/tokens.css">
  <link rel="stylesheet" href="/css/components.css">
  <link rel="stylesheet" href="/css/style.css">
</head>
<body>
  <nav id="sidebar-container"></nav>
  <main>
    <header class="page-header">
      <div class="page-header__titles">
        <h1 class="page-header__title">Your Page</h1>
        <p class="page-header__subtitle">Short description</p>
      </div>
    </header>
    <!-- content -->
  </main>
  <script src="/js/theme.js"></script>
  <script src="/js/drawer.js"></script>
  <script src="/js/sidebar.js"></script>
  <script src="/js/status.js"></script>
</body>
</html>
```

3. Register the route in `SIDEBAR_ITEMS` (`sidebar.js`).
4. Use token variables and component classes only.
5. Put page-only CSS in a dedicated file under `static/css/` if it grows beyond a small `<style>` block.

---

## Pages map

| Path | Purpose |
|---|---|
| `/` (`index.html`) | Dashboard — stats + timeline chart |
| `/search.html` | Search index |
| `/explorer.html` | Tree browser |
| `/duplicates.html` | Duplicate file groups |
| `/duplicate-folders.html` | Duplicate folder groups / merge |
| `/skipped.html` | Paths skipped during indexing |
| `/ignored.html` | Ignore rules + skipped-by-rule stats |
| `/status.html` | Live indexing status |
| `/processes.html` | Background process monitor |
| `/logs.html` | Log stream |
| `/settings.html` | Process toggles, refresh interval, ignore rules |

---

## Conventions

1. **No hardcoded colors** in page CSS — use `var(--…)` tokens.
2. **Buttons** use `.btn` (+ variant), not bare styled `<button>` rules.
3. **Tables** live inside `.table-wrap` when they should look like elevated cards.
4. **Detail UIs** use `Drawer` when possible instead of one-off fixed panels.
5. **Shared formatters** (`formatSize`, `escapeHtml`, …) live in `app.js` when reused across pages.
6. **Sidebar is the only nav source** — do not duplicate nav markup in HTML.
7. **Theme-aware charts** (dashboard) should re-render on `themechange`.

---

## Related docs

- [PROJECT.md](./PROJECT.md) — product overview and architecture
- [features.md](./features.md) — feature notes
- [DOCKER.md](./DOCKER.md) — container deployment

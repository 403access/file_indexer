const API = {
    async search(params) {
        const query = new URLSearchParams();
        if (params.name) query.set('name', params.name);
        if (params.type) query.set('type', params.type);
        if (params.pattern) query.set('pattern', params.pattern);
        if (params.sort) query.set('sort', params.sort);
        if (params.order) query.set('order', params.order);
        if (params.page) query.set('page', params.page);
        if (params.per_page) query.set('per_page', params.per_page);

        const res = await fetch(`/api/search?${query}`);
        if (!res.ok) throw new Error(`Search failed: ${res.statusText}`);
        return res.json();
    },
    async folder(path) {
        const res = await fetch(`/api/folder?path=${encodeURIComponent(path)}`);
        if (!res.ok) throw new Error(`Failed to load folder: ${res.statusText}`);
        return res.json();
    }
};

function formatSize(bytes) {
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i];
}

function formatDate(timestamp) {
    if (!timestamp) return '-';
    return new Date(timestamp * 1000).toLocaleDateString();
}

function formatDateFull(timestamp) {
    if (!timestamp) return '-';
    return new Date(timestamp * 1000).toLocaleString();
}

function truncatePath(path) {
    if (!path) return '';
    const parts = path.split('/');
    if (parts.length <= 3) return path;
    return '.../' + parts.slice(-2).join('/');
}

function escapeHtml(str) {
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

// ---------------------------------------------------------------------------
// Folder detail drawer (reusable Drawer component when available)
// ---------------------------------------------------------------------------

let folderDrawer = null;
let folderSidebar = null; // legacy fallback reference

function getFolderDrawer() {
    if (folderDrawer) return folderDrawer;
    if (typeof Drawer === 'undefined') return null;
    folderDrawer = Drawer.create({
        id: 'folder-drawer',
        title: 'Folder',
        size: 'md',
    });
    return folderDrawer;
}

function folderMetaHtml(folder, totalSize) {
    return `
        <div class="meta-row">
            <span class="meta-row__label">Path</span>
            <span class="meta-row__value meta-row__value--mono" data-folder-path title="${escapeHtml(folder.path)}">${escapeHtml(folder.path)}</span>
        </div>
        <div class="meta-row">
            <span class="meta-row__label">Size</span>
            <span class="meta-row__value" data-folder-size>${formatSize(totalSize)}</span>
        </div>
        <div class="meta-row">
            <span class="meta-row__label">Modified</span>
            <span class="meta-row__value" data-folder-modified>${formatDateFull(folder.modified)}</span>
        </div>
        <div class="meta-row">
            <span class="meta-row__label">Contents</span>
            <span class="meta-row__value" data-folder-counts>${folder.folder_count} folders, ${folder.file_count} files</span>
        </div>
    `;
}

function folderBodyHtml(folder) {
    const rows = folder.files.length === 0
        ? '<div class="empty-state"><div class="empty-state__title">Empty folder</div></div>'
        : `<div class="list">${folder.files.map(file => {
            const safePath = file.path.replace(/'/g, "\\'");
            const safeName = file.name.replace(/'/g, "\\'");
            const onclick = file.is_directory
                ? `loadSubfolder('${safePath}')`
                : `FileViewer.open('${safePath}', '${safeName}')`;
            return `
                <div class="list-row" onclick="${onclick}">
                    <span class="list-row__icon">${file.is_directory ? '\uD83D\uDCC1' : '\uD83D\uDCC4'}</span>
                    <span class="list-row__name" title="${escapeHtml(file.name)}">${escapeHtml(file.name)}</span>
                    <span class="list-row__meta">${file.is_directory ? '' : formatSize(file.size)}</span>
                </div>`;
        }).join('')}</div>`;

    return `
        <div class="drawer__section">
            <div class="drawer__section-title">Contents</div>
            ${rows}
        </div>
    `;
}

async function openFolder(path) {
    try {
        const data = await API.folder(path);
        showFolderSidebar(data);
    } catch (err) {
        console.error('Failed to load folder:', err);
    }
}

function showFolderSidebar(folder) {
    const totalSize = folder.files.reduce((sum, f) => sum + (f.is_file ? f.size : 0), 0);
    const drawer = getFolderDrawer();

    if (drawer) {
        drawer
            .setTitle(folder.name)
            .setMeta(folderMetaHtml(folder, totalSize))
            .setBody(folderBodyHtml(folder))
            .open();
        folderSidebar = { kind: 'drawer', drawer };
        return;
    }

    // Fallback without Drawer.js
    closeFolderSidebar();
    const overlay = document.createElement('div');
    overlay.className = 'folder-sidebar-overlay';
    overlay.onclick = closeFolderSidebar;

    const sidebar = document.createElement('div');
    sidebar.className = 'folder-sidebar';
    sidebar.innerHTML = `
        <div class="fs-header">
            <button type="button" class="fs-close" onclick="closeFolderSidebar()" aria-label="Close">&times;</button>
            <span class="fs-title">${escapeHtml(folder.name)}</span>
        </div>
        <div class="fs-meta">${folderMetaHtml(folder, totalSize)
            .replace(/meta-row__/g, 'fs-meta-')
            .replace(/class="meta-row"/g, 'class="fs-meta-row"')
            .replace(/meta-row__value--mono/g, 'fs-path')
            .replace(/data-folder-path/g, 'fs-path')}</div>
        <div class="fs-content">${folderBodyHtml(folder)}</div>
    `;

    document.body.appendChild(overlay);
    document.body.appendChild(sidebar);
    folderSidebar = { kind: 'legacy', overlay, sidebar };

    requestAnimationFrame(() => {
        overlay.classList.add('open');
        sidebar.classList.add('open');
    });
}

async function loadSubfolder(path) {
    try {
        const data = await API.folder(path);
        updateFolderSidebar(data);
    } catch (err) {
        console.error('Failed to load subfolder:', err);
    }
}

function updateFolderSidebar(folder) {
    if (!folderSidebar) {
        showFolderSidebar(folder);
        return;
    }

    const totalSize = folder.files.reduce((sum, f) => sum + (f.is_file ? f.size : 0), 0);

    if (folderSidebar.kind === 'drawer') {
        const d = folderSidebar.drawer;
        d.setTitle(folder.name)
            .setMeta(folderMetaHtml(folder, totalSize))
            .setBody(folderBodyHtml(folder));
        return;
    }

    const sidebar = folderSidebar.sidebar;
    const title = sidebar.querySelector('.fs-title');
    if (title) title.textContent = folder.name;
    const content = sidebar.querySelector('.fs-content');
    if (content) content.innerHTML = folderBodyHtml(folder);
    const meta = sidebar.querySelector('.fs-meta');
    if (meta) {
        meta.innerHTML = folderMetaHtml(folder, totalSize)
            .replace(/class="meta-row"/g, 'class="fs-meta-row"')
            .replace(/meta-row__label/g, 'fs-meta-label')
            .replace(/meta-row__value meta-row__value--mono/g, 'fs-meta-value fs-path')
            .replace(/meta-row__value/g, 'fs-meta-value');
    }
}

function closeFolderSidebar() {
    if (!folderSidebar) return;

    if (folderSidebar.kind === 'drawer') {
        folderSidebar.drawer.close();
        folderSidebar = null;
        return;
    }

    const { overlay, sidebar } = folderSidebar;
    overlay.classList.remove('open');
    sidebar.classList.remove('open');
    setTimeout(() => {
        overlay.remove();
        sidebar.remove();
    }, 300);
    folderSidebar = null;
}

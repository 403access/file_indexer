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

let currentPage = 1;
const perPage = 20;
let currentParams = {};

async function doSearch(page = 1) {
    currentParams = {
        name: document.getElementById('search-name').value,
        type: document.getElementById('search-type').value,
        pattern: document.getElementById('search-pattern').value,
        sort: document.getElementById('search-sort').value,
        order: document.getElementById('search-order').value,
        page: page,
        per_page: perPage
    };

    try {
        const data = await API.search(currentParams);
        currentPage = data.page;
        renderResults(data);
    } catch (err) {
        console.error(err);
    }
}

function renderResults(data) {
    const table = document.getElementById('results-table');
    const tbody = document.getElementById('results-body');
    const noResults = document.getElementById('no-results');
    const info = document.getElementById('results-info');
    const count = document.getElementById('result-count');
    const pagination = document.getElementById('pagination');

    if (data.results.length === 0) {
        table.style.display = 'none';
        noResults.style.display = 'block';
        info.style.display = 'none';
        pagination.style.display = 'none';
        return;
    }

    noResults.style.display = 'none';
    table.style.display = 'table';
    info.style.display = 'block';
    pagination.style.display = 'flex';

    count.textContent = `${data.total} result${data.total !== 1 ? 's' : ''}`;

    tbody.innerHTML = data.results.map(entry => `
        <tr class="${entry.is_directory ? 'clickable' : ''}" onclick="${entry.is_directory ? `openFolder('${entry.path.replace(/'/g, "\\'")}')` : `FileViewer.open('${entry.path.replace(/'/g, "\\'")}', '${entry.name.replace(/'/g, "\\'")}')`}">
            <td class="name">${entry.is_directory ? '\uD83D\uDCC1 ' : '\uD83D\uDCC4 '}${entry.name}</td>
            <td class="path" title="${entry.path || ''}">${truncatePath(entry.path)}</td>
            <td class="size">${entry.is_directory ? '-' : formatSize(entry.size)}</td>
            <td class="modified">${formatDate(entry.modified)}</td>
            <td class="type">${entry.is_directory ? 'Dir' : 'File'}</td>
        </tr>
    `).join('');

    const totalPages = Math.ceil(data.total / perPage);
    document.getElementById('page-info').textContent = `Page ${data.page} of ${totalPages}`;
    document.getElementById('prev-page').disabled = data.page <= 1;
    document.getElementById('next-page').disabled = data.page >= totalPages;
}

function truncatePath(path) {
    if (!path) return '';
    const parts = path.split('/');
    if (parts.length <= 3) return path;
    return '.../' + parts.slice(-2).join('/');
}

// Folder Sidebar
let folderSidebar = null;

async function openFolder(path) {
    try {
        const data = await API.folder(path);
        showFolderSidebar(data);
    } catch (err) {
        console.error('Failed to load folder:', err);
    }
}

function showFolderSidebar(folder) {
    closeFolderSidebar();

    const overlay = document.createElement('div');
    overlay.className = 'folder-sidebar-overlay';
    overlay.onclick = closeFolderSidebar;

    const sidebar = document.createElement('div');
    sidebar.className = 'folder-sidebar';

    const totalSize = folder.files.reduce((sum, f) => sum + (f.is_file ? f.size : 0), 0);

    sidebar.innerHTML = `
        <div class="fs-header">
            <button class="fs-close" onclick="closeFolderSidebar()">&times;</button>
            <span class="fs-title">${folder.name}</span>
        </div>
        <div class="fs-meta">
            <div class="fs-meta-row">
                <span class="fs-meta-label">Path</span>
                <span class="fs-meta-value fs-path" title="${folder.path}">${folder.path}</span>
            </div>
            <div class="fs-meta-row">
                <span class="fs-meta-label">Size</span>
                <span class="fs-meta-value">${formatSize(totalSize)}</span>
            </div>
            <div class="fs-meta-row">
                <span class="fs-meta-label">Modified</span>
                <span class="fs-meta-value">${formatDateFull(folder.modified)}</span>
            </div>
            <div class="fs-meta-row">
                <span class="fs-meta-label">Contents</span>
                <span class="fs-meta-value">${folder.folder_count} folders, ${folder.file_count} files</span>
            </div>
        </div>
        <div class="fs-content">
            <div class="fs-section-title">Contents</div>
            <div class="fs-file-list">
                ${folder.files.length === 0 ? '<div class="fs-empty">Empty folder</div>' : ''}
                ${folder.files.map(file => `
                    <div class="fs-file-row" onclick="${file.is_directory ? `loadSubfolder('${file.path.replace(/'/g, "\\'")}')` : `FileViewer.open('${file.path.replace(/'/g, "\\'")}', '${file.name.replace(/'/g, "\\'")}')`}">
                        <span class="fs-file-icon">${file.is_directory ? '\uD83D\uDCC1' : '\uD83D\uDCC4'}</span>
                        <span class="fs-file-name" title="${file.name}">${file.name}</span>
                        <span class="fs-file-size">${file.is_directory ? '' : formatSize(file.size)}</span>
                    </div>
                `).join('')}
            </div>
        </div>
    `;

    document.body.appendChild(overlay);
    document.body.appendChild(sidebar);
    folderSidebar = { overlay, sidebar };

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
    if (!folderSidebar) return;
    const sidebar = folderSidebar.sidebar;

    const totalSize = folder.files.reduce((sum, f) => sum + (f.is_file ? f.size : 0), 0);

    sidebar.querySelector('.fs-title').textContent = folder.name;
    sidebar.querySelector('.fs-path').textContent = folder.path;
    sidebar.querySelector('.fs-path').title = folder.path;

    const metaRows = sidebar.querySelectorAll('.fs-meta-value');
    metaRows[1].textContent = formatSize(totalSize);
    metaRows[2].textContent = formatDateFull(folder.modified);
    metaRows[3].textContent = `${folder.folder_count} folders, ${folder.file_count} files`;

    const fileList = sidebar.querySelector('.fs-file-list');
    fileList.innerHTML = folder.files.length === 0 ? '<div class="fs-empty">Empty folder</div>' :
        folder.files.map(file => `
            <div class="fs-file-row" onclick="${file.is_directory ? `loadSubfolder('${file.path.replace(/'/g, "\\'")}')` : `FileViewer.open('${file.path.replace(/'/g, "\\'")}', '${file.name.replace(/'/g, "\\'")}')`}">
                <span class="fs-file-icon">${file.is_directory ? '\uD83D\uDCC1' : '\uD83D\uDCC4'}</span>
                <span class="fs-file-name" title="${file.name}">${file.name}</span>
                <span class="fs-file-size">${file.is_directory ? '' : formatSize(file.size)}</span>
            </div>
        `).join('');
}

function closeFolderSidebar() {
    if (!folderSidebar) return;
    const { overlay, sidebar } = folderSidebar;
    overlay.classList.remove('open');
    sidebar.classList.remove('open');
    setTimeout(() => {
        overlay.remove();
        sidebar.remove();
    }, 300);
    folderSidebar = null;
}

document.getElementById('search-form').addEventListener('submit', (e) => {
    e.preventDefault();
    doSearch(1);
});

document.getElementById('prev-page').addEventListener('click', () => {
    if (currentPage > 1) doSearch(currentPage - 1);
});

document.getElementById('next-page').addEventListener('click', () => {
    doSearch(currentPage + 1);
});

document.getElementById('search-sort').addEventListener('change', () => doSearch(1));
document.getElementById('search-order').addEventListener('change', () => doSearch(1));

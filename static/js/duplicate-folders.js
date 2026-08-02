const HASH_COLORS = [
    '#e74c3c', '#3498db', '#2ecc71', '#f39c12', '#9b59b6',
    '#1abc9c', '#e67e22', '#34495e', '#16a085', '#c0392b',
    '#2980b9', '#8e44ad', '#27ae60', '#d35400', '#7f8c8d'
];
let hashColorMap = {};
let colorIdx = 0;

function hashColor(hash) {
    if (!hashColorMap[hash]) {
        hashColorMap[hash] = HASH_COLORS[colorIdx % HASH_COLORS.length];
        colorIdx++;
    }
    return hashColorMap[hash];
}

function truncatePath(p) {
    if (!p) return '';
    const parts = p.split('/');
    if (parts.length <= 3) return p;
    return '.../' + parts.slice(-3).join('/');
}

function buildMiniTree(path, commonRoot) {
    const normalized = path.replace(/\/+/g, '/');
    const rootNorm = commonRoot.replace(/\/+/g, '/').replace(/\/$/, '');
    const rel = normalized.replace(rootNorm, '').replace(/^\//, '');
    const parts = rel.split('/').filter(Boolean);

    let lines = [];
    const rootName = rootNorm.split('/').pop() || rootNorm;
    lines.push({ indent: '', connector: '', name: rootName, isCurrent: false, isRoot: true });

    for (let i = 0; i < parts.length; i++) {
        const isLast = i === parts.length - 1;
        const indent = '  '.repeat(i + 1);
        const connector = isLast ? '\u2514\u2500 ' : '\u251C\u2500 ';
        lines.push({ indent, connector, name: parts[i], isCurrent: isLast, isRoot: false });
    }

    return lines.map(l =>
        `<div class="tree-line${l.isCurrent ? ' current' : ''}">` +
        `<span class="tree-prefix">${l.indent}${l.connector}</span>` +
        `<span>${l.isRoot ? '' : (l.isCurrent ? '\uD83D\uDCC1' : '')} ${l.name}</span>` +
        `</div>`
    ).join('');
}

function toggleShowAll(el) {
    const checkbox = el.querySelector('input');
    const col = el.closest('.folder-col');
    const uniqueFiles = col.querySelectorAll('.folder-file.unique');
    uniqueFiles.forEach(f => {
        f.style.display = checkbox.checked ? 'flex' : 'none';
    });
}

function initToggles() {
    document.querySelectorAll('.show-all-toggle').forEach(el => {
        const col = el.closest('.folder-col');
        const uniqueFiles = col.querySelectorAll('.folder-file.unique');
        if (uniqueFiles.length === 0) {
            el.style.display = 'none';
        } else {
            uniqueFiles.forEach(f => f.style.display = 'none');
        }
    });
}

let groupsData = null;
let currentPage = 1;
const perPage = 20;

function applyFilter() {
    currentPage = 1;
    loadGroups();
}

async function loadGroups() {
    const q = (document.getElementById('filter-input').value || '').trim();
    const minShared = parseInt(document.getElementById('min-shared-input').value || '0', 10) || 0;
    const minFolders = parseInt(document.getElementById('min-folders').value || '0', 10) || 0;
    const fileTypes = (document.getElementById('file-types-input').value || '').trim();
    const sort = document.getElementById('sort-select').value || 'shared';
    const order = document.getElementById('sort-dir').value || 'desc';

    const params = new URLSearchParams();
    params.set('page', currentPage);
    params.set('per_page', perPage);
    if (q) params.set('q', q);
    if (minShared) params.set('min_shared', minShared);
    if (minFolders) params.set('min_folders', minFolders);
    if (fileTypes) params.set('file_types', fileTypes);
    if (sort !== 'shared') params.set('sort', sort);
    if (order !== 'desc') params.set('order', order);

    const res = await fetch(`/api/duplicate-folders?${params.toString()}`);
    if (!res.ok) return;
    groupsData = await res.json();
    renderGroups(groupsData);
    renderPagination();
}

function renderPagination() {
    const container = document.getElementById('pagination');
    const totalPages = Math.ceil(groupsData.total_groups / perPage);

    if (totalPages <= 1) {
        container.innerHTML = '';
        return;
    }

    let html = '';
    html += `<button ${currentPage === 1 ? 'disabled' : ''} onclick="goToPage(${currentPage - 1})">Previous</button>`;

    const maxVisible = 5;
    let startPage = Math.max(1, currentPage - Math.floor(maxVisible / 2));
    let endPage = Math.min(totalPages, startPage + maxVisible - 1);
    if (endPage - startPage < maxVisible - 1) {
        startPage = Math.max(1, endPage - maxVisible + 1);
    }

    if (startPage > 1) {
        html += `<button onclick="goToPage(1)">1</button>`;
        if (startPage > 2) html += `<span class="page-info">...</span>`;
    }

    for (let i = startPage; i <= endPage; i++) {
        html += `<button class="${i === currentPage ? 'active' : ''}" onclick="goToPage(${i})">${i}</button>`;
    }

    if (endPage < totalPages) {
        if (endPage < totalPages - 1) html += `<span class="page-info">...</span>`;
        html += `<button onclick="goToPage(${totalPages})">${totalPages}</button>`;
    }

    html += `<button ${currentPage === totalPages ? 'disabled' : ''} onclick="goToPage(${currentPage + 1})">Next</button>`;
    html += `<span class="page-info">${groupsData.total_groups} total groups</span>`;

    container.innerHTML = html;
}

function goToPage(page) {
    currentPage = page;
    loadGroups();
    window.scrollTo(0, 0);
}

function renderGroups(data) {
    const container = document.getElementById('groups');
    const summary = document.getElementById('summary');
    const aligned = document.getElementById('align-files').checked;

    if (data.groups.length === 0) {
        container.innerHTML = '<div class="empty-msg">No duplicate folders found</div>';
        summary.textContent = '';
        return;
    }

    let totalFolders = 0;
    data.groups.forEach(g => totalFolders += g.folders.length);
    summary.textContent = `${data.groups.length} group${data.groups.length !== 1 ? 's' : ''} \u2022 ${totalFolders} folders compared`;

    container.innerHTML = data.groups.map((group, gi) => {
        const paths = group.folders.map(f => f.path.replace(/\/+/g, '/'));
        let commonRoot = paths[0] || '';
        for (const p of paths) {
            while (!p.startsWith(commonRoot) && commonRoot.length > 0) {
                commonRoot = commonRoot.substring(0, commonRoot.lastIndexOf('/'));
            }
        }

        const folderContent = aligned
            ? renderAlignedTable(group, gi)
            : renderFolderColumns(group, gi, commonRoot);

        return `
            <div class="group-card">
                <div class="group-header">
                    <span class="badge">${group.shared_count} shared</span>
                    <span class="label">${group.folders.length} folders with duplicate content</span>
                    <button class="merge-folders-btn" onclick="openMergeFolders(${gi})">Merge</button>
                </div>
                ${folderContent}
            </div>
        `;
    }).join('');

    if (!aligned) initToggles();
}

function renderFolderColumns(group, gi, commonRoot) {
    return `<div class="folders-row">
        ${group.folders.map((folder, fi) => `
            <div class="folder-col" data-gi="${gi}" data-fi="${fi}" data-path="${folder.path}">
                <div class="folder-col-header">
                    ${folder.name}
                    <span class="full-path" title="${folder.path}">${truncatePath(folder.path)}</span>
                </div>
                <div class="mini-tree">${buildMiniTree(folder.path, commonRoot)}</div>
                <div class="folder-col-files">
                    <div class="folder-load-btn" onclick="loadFolderFiles(this, '${folder.path.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}', ${gi}, ${fi})" style="padding:1rem;text-align:center;color:#3498db;cursor:pointer;font-size:0.85rem;">
                        Click to load ${folder.file_count} files
                    </div>
                </div>
            </div>
        `).join('')}
    </div>`;
}

function renderAlignedTable(group, gi) {
    return `<div style="padding:1rem;text-align:center;">
        <div class="folder-load-btn" onclick="loadAlignedFiles(this, ${gi})" style="color:#3498db;cursor:pointer;font-size:0.85rem;">
            Click to load files for aligned comparison
        </div></div>`;
}

async function loadFolderFiles(btn, path, gi, fi) {
    btn.textContent = 'Loading...';
    btn.style.color = '#999';
    btn.style.cursor = 'default';
    btn.onclick = null;

    try {
        const res = await fetch(`/api/duplicate-folders/files?path=${encodeURIComponent(path)}`);
        if (!res.ok) throw new Error('Failed to load');
        const data = await res.json();

        const container = btn.parentElement;
        container.innerHTML = data.files.map(file => `
            <div class="folder-file ${file.is_duplicate ? 'shared' : 'unique'}" ${!file.is_duplicate ? 'data-unique="true"' : ''} style="cursor:pointer" onclick="FileViewer.open('${path.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}/${file.name}', '${file.name.replace(/'/g, "\\'")}')">
                <span class="hash-dot" style="background:${file.is_duplicate ? hashColor(file.hash) : '#ccc'}"></span>
                <span class="file-icon">\uD83D\uDCC4</span>
                <span class="file-name" title="${file.name}">${file.name}</span>
                <span class="file-size">${formatSize(file.size)}</span>
            </div>
        `).join('');
    } catch (e) {
        btn.textContent = 'Failed to load';
        btn.style.color = '#e74c3c';
    }
}

async function loadAlignedFiles(btn, gi) {
    btn.textContent = 'Loading...';
    btn.style.color = '#999';
    btn.style.cursor = 'default';
    btn.onclick = null;

    const group = groupsData.groups[gi];
    const folders = group.folders;

    try {
        const allFiles = await Promise.all(folders.map(async (folder) => {
            const res = await fetch(`/api/duplicate-folders/files?path=${encodeURIComponent(folder.path)}`);
            if (!res.ok) return [];
            const data = await res.json();
            return data.files;
        }));

        const folderCount = folders.length;
        const byHash = {};
        folders.forEach((folder, fi) => {
            allFiles[fi].forEach(file => {
                if (!byHash[file.hash]) byHash[file.hash] = [];
                byHash[file.hash].push({ ...file, folderIdx: fi, folderPath: folder.path, folderName: folder.name });
            });
        });

        const hashEntries = Object.entries(byHash).sort((a, b) => {
            const aShared = a[1].length > 1 ? 0 : 1;
            const bShared = b[1].length > 1 ? 0 : 1;
            if (aShared !== bShared) return aShared - bShared;
            return a[1][0].name.localeCompare(b[1][0].name);
        });

        const rows = hashEntries.map(([hash, files]) => {
            const cells = new Array(folderCount).fill(null);
            files.forEach(f => { cells[f.folderIdx] = f; });
            return { hash, cells, isShared: files.length > 1 };
        });

        let html = '<table class="aligned-table"><thead><tr>';
        html += '<th style="width:24px"></th>';
        folders.forEach(folder => {
            html += `<th>${folder.name}<span class="th-path" title="${folder.path}">${truncatePath(folder.path)}</span></th>`;
        });
        html += '</tr></thead><tbody>';

        rows.forEach(row => {
            html += '<tr>';
            html += `<td><span class="hash-dot" style="background:${row.isShared ? hashColor(row.hash) : '#ccc'}"></span></td>`;
            row.cells.forEach(cell => {
                if (cell) {
                    html += `<td><div class="file-cell" onclick="FileViewer.open('${cell.folderPath.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}/${cell.name}', '${cell.name.replace(/'/g, "\\'")}')">`;
                    html += `<span class="file-name" title="${cell.name}">${cell.name}</span>`;
                    html += `<span class="file-size">${formatSize(cell.size)}</span>`;
                    html += '</div></td>';
                } else {
                    html += '<td class="empty-cell">\u2014</td>';
                }
            });
            html += '</tr>';
        });

        html += '</tbody></table>';
        btn.parentElement.innerHTML = html;
    } catch (e) {
        btn.textContent = 'Failed to load';
        btn.style.color = '#e74c3c';
    }
}

let mergeGroup = null;
let mergeSelections = {};
let mergeViewMode = 'tree';

function setMergeView(mode) {
    mergeViewMode = mode;
    document.getElementById('view-summary-btn').classList.toggle('active', mode === 'summary');
    document.getElementById('view-list-btn').classList.toggle('active', mode === 'list');
    document.getElementById('view-tree-btn').classList.toggle('active', mode === 'tree');
    renderMergeBody();
}

function parseCustomFolders() {
    const raw = document.getElementById('custom-folders-input').value || '';
    return raw.split('\n')
        .map(l => l.trim())
        .filter(Boolean)
        .map(p => p.replace(/\/+$/, ''));
}

let customFolderListMode = false;

function toggleFolderInputMode() {
    customFolderListMode = !customFolderListMode;
    syncCustomFoldersBetweenModes();
    const paste = document.getElementById('custom-folders-paste-mode');
    const list = document.getElementById('custom-folders-list-mode');
    const text = document.getElementById('custom-folders-mode-text');
    if (customFolderListMode) {
        paste.style.display = 'none';
        list.style.display = '';
        text.textContent = 'Folder list';
    } else {
        paste.style.display = '';
        list.style.display = 'none';
        text.textContent = 'Paste list';
    }
}

function syncCustomFoldersBetweenModes() {
    if (customFolderListMode) {
        // Entering list mode: build one row per path from the textarea
        const paths = parseCustomFolders();
        document.getElementById('custom-folder-rows').innerHTML = '';
        (paths.length ? paths : ['']).forEach(p => addFolderRow(p));
    } else {
        // Returning to paste mode: rebuild the textarea from the rows
        document.getElementById('custom-folders-input').value = getCustomFolderRows().join('\n');
    }
}

function getCustomFolderRows() {
    return Array.from(document.querySelectorAll('#custom-folder-rows input[type="text"]'))
        .map(i => i.value.trim())
        .filter(Boolean)
        .map(p => p.replace(/\/+$/, ''));
}

function addFolderRow(path) {
    const val = escapeHtml(path || '');
    const container = document.getElementById('custom-folder-rows');
    const row = document.createElement('div');
    row.className = 'custom-folder-row';
    row.innerHTML =
        '<input type="text" placeholder="/path/to/folder" value="' + val + '">' +
        '<button type="button" class="custom-folder-remove" title="Remove folder" onclick="removeFolderRow(this)">&times;</button>';
    container.appendChild(row);
}

function removeFolderRow(btn) {
    btn.closest('.custom-folder-row').remove();
}

function getCustomFolderPaths() {
    return customFolderListMode ? getCustomFolderRows() : parseCustomFolders();
}

function escapeHtml(str) {
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function openMergeOverlay() {
    const overlay = document.getElementById('merge-overlay');
    overlay.style.display = 'block';
    requestAnimationFrame(() => overlay.classList.add('open'));
}

async function checkCustomFolders() {
    const paths = getCustomFolderPaths();
    if (paths.length < 2) {
        showMergeError('Please provide at least two folder paths.', []);
        return;
    }

    // Check which provided folders actually exist on disk
    let results = paths.map(p => ({ path: p, resolved: p, exists: true, is_dir: true }));
    try {
        const res = await fetch('/api/folders/check', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ paths })
        });
        if (res.ok) {
            const data = await res.json();
            results = data.results || results;
        }
    } catch (e) {
        console.error('Failed to check folders:', e);
    }

    const missing = results.filter(r => !r.exists || !r.is_dir);
    if (missing.length > 0) {
        showMergeError(
            'The following folder(s) do not exist or are not directories.',
            missing.map(r => r.path)
        );
        return;
    }

    const folders = results.map(r => {
        const normalized = r.resolved.replace(/\/+$/, '');
        const name = normalized.split('/').filter(Boolean).pop() || r.path;
        return { path: normalized, name, file_count: 0 };
    });
    await openMergeForFolders(folders);
}

function showMergeError(title, paths) {
    mergeGroup = null;
    mergeSelections = {};

    const body = document.getElementById('merge-body');
    body.className = 'merge-body';
    body.innerHTML = `
        <div class="merge-error">
            <div class="merge-error-title">${escapeHtml(title)}</div>
            ${paths.length
                ? `<div class="merge-error-list">${paths.map(p =>
                    `<div class="merge-error-item">${escapeHtml(p)}</div>`).join('')}</div>`
                : ''}
            <div class="merge-error-hint">Fix the paths in the text box (one per line) and click Check &amp; Merge again.</div>
        </div>`;

    const stats = document.getElementById('merge-stats');
    stats.textContent = paths.length
        ? `${paths.length} invalid folder${paths.length !== 1 ? 's' : ''}`
        : '';

    const btn = document.getElementById('merge-apply');
    btn.disabled = true;
    btn.textContent = 'Fix paths';

    openMergeOverlay();
}

async function openMergeFolders(groupIdx) {
    const group = groupsData.groups[groupIdx];
    if (!group || group.folders.length < 2) return;
    await openMergeForFolders(group.folders);
}

async function openMergeForFolders(folders) {
    mergeGroup = { folders };
    mergeSelections = {};
    document.getElementById('merge-dest-path').value = (folders[0]?.path || '').replace(/\/[^/]+$/, '') + '/merged';
    renderMergeBody();
    openMergeOverlay();

    try {
        const allFiles = await Promise.all(folders.map(async (folder) => {
            const res = await fetch(`/api/duplicate-folders/files?path=${encodeURIComponent(folder.path)}`);
            if (!res.ok) return { path: folder.path, name: folder.name, files: [] };
            const data = await res.json();
            return { path: folder.path, name: folder.name, files: data.files };
        }));

        // Only consider folders that have indexed files
        const nonEmpty = allFiles.filter(f => f.files.length > 0);
        mergeGroup.foldersWithFiles = nonEmpty;

        const byHash = {};
        nonEmpty.forEach(({ path, name, files }) => {
            files.forEach(file => {
                if (!byHash[file.hash]) byHash[file.hash] = [];
                byHash[file.hash].push({ ...file, folderPath: path, folderName: name });
            });
        });

        mergeSelections = {};
        Object.entries(byHash).forEach(([hash, files]) => {
            const isShared = files.length > 1;
            files.forEach((f, idx) => {
                const key = f.folderPath + '/' + f.name;
                mergeSelections[key] = isShared ? idx === 0 : true;
            });
        });

        renderMergeBody();
    } catch (e) {
        console.error('Failed to load merge files:', e);
    }
}

function renderMergeBody() {
    const body = document.getElementById('merge-body');
    if (!mergeGroup.foldersWithFiles) {
        body.innerHTML = '<div style="padding:2rem;text-align:center;color:#999;">Loading files...</div>';
        body.className = 'merge-body';
        updateMergeStats();
        return;
    }
    if (mergeViewMode === 'summary') {
        renderMergeSummaryTab(body);
    } else if (mergeViewMode === 'tree') {
        renderMergeTree(body);
    } else {
        renderMergeList(body);
    }
    updateMergeStats();
}

function renderMergeSummaryTab(body) {
    let html = '<div class="merge-summary-folders">';
    mergeGroup.foldersWithFiles.forEach(({ path, name, files }) => {
        let kept = 0, removed = 0, totalSize = 0;
        files.forEach(file => {
            const key = path + '/' + file.name;
            totalSize += file.size;
            if (mergeSelections[key]) kept++;
            else removed++;
        });
        html += `
            <div class="merge-summary-folder">
                <span class="folder-icon">\uD83D\uDCC1</span>
                <span class="folder-name">${name}</span>
                <span class="folder-path">${path}</span>
                <span class="folder-stats">
                    <span class="stat-total">${files.length} files (${formatSize(totalSize)})</span>
                    <span class="stat-kept">\u2713${kept}</span>
                    <span class="stat-removed">\u2717${removed}</span>
                </span>
            </div>`;
    });
    html += '</div>';
    body.innerHTML = html;
    body.className = 'merge-body';
}

function renderMergeList(body) {
    const byHash = {};
    mergeGroup.foldersWithFiles.forEach(({ path, name, files }) => {
        files.forEach(file => {
            if (!byHash[file.hash]) byHash[file.hash] = [];
            byHash[file.hash].push({
                ...file,
                folderPath: path,
                folderName: name
            });
        });
    });

    const entries = Object.entries(byHash).sort((a, b) => {
        const aShared = a[1].length > 1 ? 0 : 1;
        const bShared = b[1].length > 1 ? 0 : 1;
        return aShared - bShared;
    });

    let html = '';
    entries.forEach(([hash, files]) => {
        const isShared = files.length > 1;
        const label = isShared ? `Shared (${files.length} copies)` : 'Unique';
        html += `<div class="merge-dir-group">
            <div class="merge-dir-label">${label}</div>`;
        files.forEach(f => {
            const key = f.folderPath + '/' + f.name;
            const isKeep = mergeSelections[key];
            html += `
                <div class="merge-file-row ${isKeep ? 'keep' : 'remove'}">
                    <span class="merge-file-hash" style="background:${hashColor(hash)}"></span>
                    <input type="checkbox" class="merge-file-check"
                        ${isKeep ? 'checked' : ''}
                        onchange="toggleMergeFile('${key.replace(/'/g, "\\'")}', this.checked)">
                    <span class="merge-file-name">${f.name}</span>
                    <span class="merge-file-path" title="${f.folderPath}/${f.name}">${truncatePath(f.folderPath)}/${f.name}</span>
                    <span class="merge-file-size">${formatSize(f.size)}</span>
                </div>`;
        });
        html += '</div>';
    });
    body.innerHTML = html;
    body.className = 'merge-body';
}

function renderMergeTree(body) {
    const dest = document.getElementById('merge-dest-path').value.trim() || 'merged';
    const destName = dest.split('/').filter(Boolean).pop() || 'merged';

    const tree = { name: destName, children: {}, isDir: true };
    const allFiles = [];

    mergeGroup.foldersWithFiles.forEach(({ path, files }) => {
        files.forEach(file => {
            const key = path + '/' + file.name;
            const isKeep = mergeSelections[key];
            allFiles.push({ ...file, folderPath: path, key, isKeep });
        });
    });

    allFiles.sort((a, b) => {
        if (a.isKeep !== b.isKeep) return a.isKeep ? -1 : 1;
        return a.name.localeCompare(b.name);
    });

    allFiles.forEach(f => {
        if (!f.isKeep) return;
        const relPath = f.folderPath.replace(/\/+/g, '/');
        const parts = relPath.split('/').filter(Boolean);
        let node = tree;
        parts.forEach(part => {
            if (!node.children[part]) {
                node.children[part] = { name: part, children: {}, isDir: true };
            }
            node = node.children[part];
        });
        if (!node.children[f.name]) {
            node.children[f.name] = { name: f.name, isDir: false, size: f.size, hash: f.hash };
        }
    });

    let html = '<div class="merge-tree">';
    html += renderTreeNode(tree, 0, true);
    html += '</div>';
    body.innerHTML = html;
    body.className = 'merge-body';
}

function renderTreeNode(node, depth, isLast) {
    let html = '';
    const connector = isLast ? '\u2514\u2500 ' : '\u251C\u2500 ';
    const icon = node.isDir ? '\uD83D\uDCC1' : '\uD83D\uDCC4';
    const size = node.size ? `<span class="tree-size">${formatSize(node.size)}</span>` : '';
    const dot = node.hash ? `<span class="tree-hash-dot" style="background:${hashColor(node.hash)}"></span>` : '';

    html += `<div class="tree-line" style="padding-left:${depth * 1.2}rem">`;
    html += `<span class="tree-prefix">${connector}</span>`;
    html += `${dot}<span class="tree-icon">${icon}</span>`;
    html += `<span class="tree-name">${node.name}</span>`;
    html += `${size}`;
    html += `</div>`;

    if (node.isDir && node.children) {
        const entries = Object.values(node.children);
        entries.forEach((child, i) => {
            html += renderTreeNode(child, depth + 1, i === entries.length - 1);
        });
    }
    return html;
}

function toggleMergeFile(key, checked) {
    mergeSelections[key] = checked;
    renderMergeBody();
}

function updateMergeStats() {
    const kept = Object.values(mergeSelections).filter(v => v).length;
    const removed = Object.values(mergeSelections).filter(v => !v).length;
    if (!mergeGroup.foldersWithFiles) {
        document.getElementById('merge-stats').textContent = 'Loading...';
        return;
    }
    const totalSize = mergeGroup.foldersWithFiles.reduce((sum, f) =>
        sum + f.files.reduce((s, file) => s + file.size, 0), 0);
    const freedSize = mergeGroup.foldersWithFiles.reduce((sum, f) =>
        sum + f.files.filter(file => !mergeSelections[f.path + '/' + file.name])
            .reduce((s, file) => s + file.size, 0), 0);
    document.getElementById('merge-stats').textContent =
        `Keeping ${kept} file${kept !== 1 ? 's' : ''}, removing ${removed} (${formatSize(freedSize)} freed)`;
}

function closeMerge() {
    const overlay = document.getElementById('merge-overlay');
    overlay.classList.remove('open');
    setTimeout(() => { overlay.style.display = 'none'; }, 200);
    mergeGroup = null;
    mergeSelections = {};
    mergeViewMode = 'tree';
    document.getElementById('view-summary-btn').classList.remove('active');
    document.getElementById('view-list-btn').classList.remove('active');
    document.getElementById('view-tree-btn').classList.add('active');
    const btn = document.getElementById('merge-apply');
    btn.disabled = false;
    btn.textContent = 'Apply Merge';
}

async function applyMerge() {
    const dest = document.getElementById('merge-dest-path').value.trim();
    if (!dest) { alert('Please set a destination path.'); return; }

    const keep = Object.entries(mergeSelections)
        .filter(([_, v]) => v)
        .map(([key]) => key);

    const remove = Object.entries(mergeSelections)
        .filter(([_, v]) => !v)
        .map(([key]) => key);

    if (keep.length === 0) { alert('Keep at least one file.'); return; }

    const btn = document.getElementById('merge-apply');
    btn.disabled = true;
    btn.textContent = 'Merging...';

    try {
        const res = await fetch('/api/merge', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ keep, remove, destination: dest })
        });
        if (!res.ok) {
            const err = await res.text();
            alert('Merge failed: ' + err);
        } else {
            const result = await res.json();
            alert(`Merge complete: ${result.copied} copied, ${result.removed} removed.`);
            closeMerge();
            loadGroups();
        }
    } catch (e) {
        alert('Error: ' + e.message);
    } finally {
        btn.disabled = false;
        btn.textContent = 'Apply Merge';
    }
}

document.getElementById('merge-overlay').addEventListener('click', (e) => {
    if (e.target === e.currentTarget) closeMerge();
});

loadGroups();

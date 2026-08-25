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
    const minSize = parseInt(document.getElementById('min-size-input').value || '0', 10) || 0;
    const minSizeUnit = parseInt(document.getElementById('min-size-unit').value || '1', 10) || 1;
    const fileTypes = Array.from(selectedFileTypes).join(',');
    const sort = document.getElementById('sort-select').value || 'shared';
    const order = document.getElementById('sort-dir').value || 'desc';

    const params = new URLSearchParams();
    params.set('page', currentPage);
    params.set('per_page', perPage);
    if (q) params.set('q', q);
    if (minShared) params.set('min_shared', minShared);
    if (minFolders) params.set('min_folders', minFolders);
    if (minSize > 0) params.set('min_size', minSize * minSizeUnit);
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
        if (data.needs_refresh) {
            container.innerHTML = '<div class="empty-msg">Duplicate folder groups are still being computed. Refresh this page shortly to see results.</div>';
        } else {
            container.innerHTML = '<div class="empty-msg">No duplicate folders found</div>';
        }
        summary.textContent = '';
        return;
    }

    let totalFolders = 0;
    data.groups.forEach(g => totalFolders += g.folders.length);
    summary.textContent = `${data.groups.length} group${data.groups.length !== 1 ? 's' : ''} • ${totalFolders} folders compared`;

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
                <span class="file-icon">📄</span>
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

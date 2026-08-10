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

function openMergeOverlay() {
    const overlay = document.getElementById('merge-overlay');
    overlay.style.display = 'block';
    requestAnimationFrame(() => overlay.classList.add('open'));
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
        const allSelected = kept === files.length;
        const noneSelected = kept === 0;
        html += `
            <div class="merge-summary-folder">
                <input type="checkbox" class="merge-folder-check"
                    ${allSelected ? 'checked' : ''}
                    ${noneSelected && files.length > 0 ? 'style="opacity:0.3"' : ''}
                    onchange="selectAllFromFolder('${path.replace(/'/g, "\\'")}', this.checked)">
                <span class="folder-icon">📁</span>
                <span class="folder-name">${name}</span>
                <span class="folder-path">${path}</span>
                <span class="folder-stats">
                    <span class="stat-total">${files.length} files (${formatSize(totalSize)})</span>
                    <span class="stat-kept">✓${kept}</span>
                    <span class="stat-removed">✗${removed}</span>
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

    const folderOrder = mergeGroup.foldersWithFiles.map(f => f.path);
    const folderNames = {};
    mergeGroup.foldersWithFiles.forEach(f => { folderNames[f.path] = f.name; });

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

    html = renderMergeListFolderHeaders(html, folderOrder, folderNames);
    body.innerHTML = html;
    body.className = 'merge-body';
}

function renderMergeListFolderHeaders(html, folderOrder, folderNames) {
    const folderHeaderRegex = /(<div class="merge-file-row[^>]*>[\s\S]*?<span class="merge-file-path" title="([^"]+)">[^<]+<\/span>[\s\S]*?<\/div>)/g;
    const folderFileMap = {};
    let match;
    while ((match = folderHeaderRegex.exec(html)) !== null) {
        const fullMatch = match[0];
        const pathTitle = match[2];
        const folderPath = pathTitle.replace(/\/[^/]+$/, '');
        if (!folderFileMap[folderPath]) folderFileMap[folderPath] = [];
        folderFileMap[folderPath].push(fullMatch);
    }

    if (Object.keys(folderFileMap).length === 0) return html;

    let result = '';
    folderOrder.forEach(folderPath => {
        const files = folderFileMap[folderPath];
        if (!files || files.length === 0) return;
        const name = folderNames[folderPath] || folderPath.split('/').filter(Boolean).pop();
        const kept = files.filter(f => f.includes('keep')).length;
        const total = files.length;
        const allSelected = kept === total;
        result += `
            <div class="merge-folder-section">
                <div class="merge-folder-header">
                    <input type="checkbox" class="merge-folder-check"
                        ${allSelected ? 'checked' : ''}
                        onchange="selectAllFromFolder('${folderPath.replace(/'/g, "\\'")}', this.checked)">
                    <span class="merge-folder-icon">📁</span>
                    <span class="merge-folder-name">${name}</span>
                    <span class="merge-folder-path">${folderPath}</span>
                    <span class="merge-folder-count">${kept}/${total} selected</span>
                </div>`;
        files.forEach(f => { result += f; });
        result += '</div>';
        delete folderFileMap[folderPath];
    });

    Object.entries(folderFileMap).forEach(([folderPath, files]) => {
        const name = folderNames[folderPath] || folderPath.split('/').filter(Boolean).pop();
        const kept = files.filter(f => f.includes('keep')).length;
        const total = files.length;
        const allSelected = kept === total;
        result += `
            <div class="merge-folder-section">
                <div class="merge-folder-header">
                    <input type="checkbox" class="merge-folder-check"
                        ${allSelected ? 'checked' : ''}
                        onchange="selectAllFromFolder('${folderPath.replace(/'/g, "\\'")}', this.checked)">
                    <span class="merge-folder-icon">📁</span>
                    <span class="merge-folder-name">${name}</span>
                    <span class="merge-folder-path">${folderPath}</span>
                    <span class="merge-folder-count">${kept}/${total} selected</span>
                </div>`;
        files.forEach(f => { result += f; });
        result += '</div>';
    });

    return result;
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
    const connector = isLast ? '└─ ' : '├─ ';
    const icon = node.isDir ? '📁' : '📄';
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

function selectAllFromFolder(folderPath, checked) {
    if (!mergeGroup || !mergeGroup.foldersWithFiles) return;
    mergeGroup.foldersWithFiles.forEach(({ path, files }) => {
        if (path === folderPath) {
            files.forEach(file => {
                const key = path + '/' + file.name;
                mergeSelections[key] = checked;
            });
        }
    });
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

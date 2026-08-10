let customFolderListMode = false;

function parseCustomFolders() {
    const raw = document.getElementById('custom-folders-input').value || '';
    return raw.split('\n')
        .map(l => l.trim())
        .filter(Boolean)
        .map(p => p.replace(/\/+$/, ''));
}

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
        const paths = parseCustomFolders();
        document.getElementById('custom-folder-rows').innerHTML = '';
        (paths.length ? paths : ['']).forEach(p => addFolderRow(p));
    } else {
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

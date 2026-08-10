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
        const connector = isLast ? '└─ ' : '├─ ';
        lines.push({ indent, connector, name: parts[i], isCurrent: isLast, isRoot: false });
    }

    return lines.map(l =>
        `<div class="tree-line${l.isCurrent ? ' current' : ''}>` +
        `<span class="tree-prefix">${l.indent}${l.connector}</span>` +
        `<span>${l.isRoot ? '' : (l.isCurrent ? '📁' : '')} ${l.name}</span>` +
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

function escapeHtml(str) {
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

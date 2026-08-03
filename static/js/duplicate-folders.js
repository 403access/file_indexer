async function checkCustomFolders() {
    const paths = getCustomFolderPaths();
    if (paths.length < 2) {
        showMergeError('Please provide at least two folder paths.', []);
        return;
    }

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

document.getElementById('merge-overlay').addEventListener('click', (e) => {
    if (e.target === e.currentTarget) closeMerge();
});

loadGroups();

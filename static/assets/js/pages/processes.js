let allProcesses = [];
let allDisabledTypes = [];

async function loadProcesses() {
    try {
        const res = await fetch('/api/processes');
        if (!res.ok) throw new Error('Failed to load processes');
        const data = await res.json();
        allProcesses = data.processes || [];
        allDisabledTypes = data.disabled_types || [];
        updateCategoryOptions(allProcesses);
        renderDisabledTypes();
        applyProcessFilters();
    } catch (e) {
        document.getElementById('processes-tbody').innerHTML =
            `<tr><td colspan="8" class="empty-msg">Failed to load: ${escapeHtml(e.message)}</td></tr>`;
    }
}

function getProcessQuery() {
    const searchEl = document.getElementById('processes-search');
    const filterEl = document.getElementById('processes-filter');
    const categoryEl = document.getElementById('processes-category');
    const sortEl = document.getElementById('processes-sort');
    return {
        q: (searchEl?.value || '').trim().toLowerCase(),
        status: filterEl?.value || 'all',
        category: categoryEl?.value || 'all',
        sort: sortEl?.value || 'id-desc',
    };
}

function updateCategoryOptions(processes) {
    const select = document.getElementById('processes-category');
    if (!select) return;
    const current = select.value || 'all';
    const cats = [...new Set(processes.map((p) => p.category).filter(Boolean))].sort((a, b) =>
        a.localeCompare(b)
    );
    select.innerHTML =
        '<option value="all">All categories</option>' +
        cats.map((c) => `<option value="${escapeAttr(c)}">${escapeHtml(c)}</option>`).join('');
    if (cats.includes(current) || current === 'all') {
        select.value = current;
    } else {
        select.value = 'all';
    }
}

function applyProcessFilters() {
    const { q, status, category, sort } = getProcessQuery();
    let list = allProcesses.slice();

    if (status === 'paused') {
        list = list.filter((p) => p.paused);
    } else if (status !== 'all') {
        list = list.filter((p) => p.status === status);
    }

    if (category !== 'all') {
        list = list.filter((p) => p.category === category);
    }

    if (q) {
        list = list.filter((p) => {
            const hay = [
                String(p.id),
                p.name || '',
                p.category || '',
                p.status || '',
                p.message || '',
            ]
                .join(' ')
                .toLowerCase();
            return hay.includes(q);
        });
    }

    const [field, dir] = sort.split('-');
    const mult = dir === 'desc' ? -1 : 1;
    list.sort((a, b) => {
        if (field === 'id') {
            return ((a.id || 0) - (b.id || 0)) * mult;
        }
        if (field === 'started') {
            const at = a.started_at ? Date.parse(a.started_at) || 0 : 0;
            const bt = b.started_at ? Date.parse(b.started_at) || 0 : 0;
            return (at - bt) * mult;
        }
        const av = String(a[field] || '').toLowerCase();
        const bv = String(b[field] || '').toLowerCase();
        return av.localeCompare(bv) * mult;
    });

    renderProcesses(list, allProcesses.length);
}

function renderProcesses(processes, totalAll = processes.length) {
    const tbody = document.getElementById('processes-tbody');
    const summary = document.getElementById('processes-summary');
    const cardsContainer = document.getElementById('process-cards');

    const source = allProcesses;
    const running = source.filter((p) => p.status === 'active').length;
    const pending = source.filter((p) => p.status === 'pending').length;
    const completed = source.filter((p) => p.status === 'completed').length;
    const failed = source.filter((p) => p.status === 'failed').length;

    const hasFilter = processes.length !== totalAll
        || (document.getElementById('processes-search')?.value || '').trim() !== ''
        || (document.getElementById('processes-filter')?.value || 'all') !== 'all'
        || (document.getElementById('processes-category')?.value || 'all') !== 'all';

    if (hasFilter) {
        summary.textContent =
            `Showing ${processes.length} of ${totalAll} • ${running} running • ${pending} upcoming • ${completed} completed • ${failed} failed`;
    } else {
        summary.textContent =
            `${totalAll} total • ${running} running • ${pending} upcoming • ${completed} completed • ${failed} failed`;
    }

    // Active cards still only show running/pending from the *filtered* set
    const active = processes.filter((p) => p.status === 'active' || p.status === 'pending');

    if (active.length === 0) {
        cardsContainer.innerHTML = '';
    } else {
        cardsContainer.innerHTML = active
            .map((p) => {
                const progressHtml =
                    p.progress !== null && p.progress !== undefined
                        ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'active' && !p.paused ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
                        : '<span class="process-mono">—</span>';

                return `<div class="process-card ${p.status}${p.paused ? ' paused' : ''}" onclick="openProcessSidebar(${p.id})">
                <div class="process-card-header">
                    <div class="process-card-header__main">
                        <div class="process-card-name" title="${escapeHtml(p.name)}">${escapeHtml(p.name)}</div>
                        <div class="process-card-meta">
                            <span class="category-badge">${escapeHtml(p.category)}</span>
                            <span class="status-badge ${p.status}">${p.status}${p.paused ? ' (paused)' : ''}</span>
                        </div>
                    </div>
                    <div class="process-card-actions" onclick="event.stopPropagation()">
                        <button type="button" class="process-action-btn trigger-btn" onclick="event.stopPropagation(); triggerProcess(${p.id})" title="Run this process immediately">▶ Trigger</button>
                        ${
                            p.paused
                                ? `<button type="button" class="process-action-btn resume-btn" onclick="event.stopPropagation(); resumeProcess(${p.id})" title="Resume the paused process">▶ Resume</button>`
                                : `<button type="button" class="process-action-btn pause-btn" onclick="event.stopPropagation(); pauseProcess(${p.id})" title="Temporarily suspend; you can resume later">⏸ Pause</button>`
                        }
                        <button type="button" class="process-action-btn stop-btn" onclick="event.stopPropagation(); stopProcess(${p.id})" title="Permanently terminate this process">⏹ Stop</button>
                    </div>
                </div>
                <div class="process-card-body">
                    <div style="flex:1;min-width:120px">${progressHtml}</div>
                    <div class="process-card-message" title="${escapeHtml(p.message || '')}">${escapeHtml(p.message || '—')}</div>
                </div>
                <div class="process-card-footer">
                    <span class="process-mono">Started: ${formatDate(p.started_at)}</span>
                    <span class="process-mono">#${p.id}</span>
                </div>
            </div>`;
            })
            .join('');
    }

    if (processes.length === 0) {
        tbody.innerHTML = hasFilter
            ? '<tr><td colspan="8" class="empty-msg">No processes match your search or filters</td></tr>'
            : '<tr><td colspan="8" class="empty-msg">No processes tracked yet</td></tr>';
        return;
    }

    tbody.innerHTML = processes
        .map((p) => {
            const progressHtml =
                p.progress !== null && p.progress !== undefined
                    ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'active' && !p.paused ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
                    : '<span class="process-mono">—</span>';

            return `<tr onclick="openProcessSidebar(${p.id})" style="cursor:pointer">
            <td class="process-mono">#${p.id}</td>
            <td><strong>${escapeHtml(p.name)}</strong></td>
            <td><span class="category-badge">${escapeHtml(p.category)}</span></td>
            <td><span class="status-badge ${p.status}">${p.status}${p.paused ? ' (paused)' : ''}</span></td>
            <td style="min-width:120px">${progressHtml}</td>
            <td style="max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title="${escapeHtml(p.message || '')}">${escapeHtml(p.message || '—')}</td>
            <td class="process-mono">${formatDate(p.started_at)}</td>
            <td class="process-mono">${formatDate(p.finished_at)}</td>
        </tr>`;
        })
        .join('');
}

function formatDate(ts) {
    if (!ts) return '—';
    try {
        const d = new Date(ts);
        if (isNaN(d.getTime())) return ts;
        return d.toLocaleTimeString();
    } catch (e) {
        return ts;
    }
}

function renderDisabledTypes() {
    const container = document.getElementById('disabled-process-types');
    if (!container) return;

    if (!allDisabledTypes.length) {
        container.innerHTML = '';
        return;
    }

    const stopReasons = {
        startup_indexing: 'ENABLE_STARTUP_INDEXING',
        dashboard_refresh: 'ENABLE_DASHBOARD_REFRESH / ENABLE_INITIAL_DASHBOARD_REFRESH',
        duplicate_folder_groups_refresh: 'ENABLE_DUPLICATE_FOLDER_GROUPS_REFRESH',
    };

    container.innerHTML = `
        <div class="disabled-types-heading">Disabled process types</div>
        <div class="disabled-types-list">
            ${allDisabledTypes.map((t) => {
                const stoppedOnly = t.stopped && !t.env_enabled;
                const envOnly = !t.stopped && !t.env_enabled;
                const stoppedAndEnv = t.stopped && t.env_enabled;
                const badge = stoppedOnly
                    ? '<span class="status-badge disabled">stopped</span>'
                    : '<span class="status-badge disabled">disabled</span>';
                const reason = stoppedOnly
                    ? 'Stopped previously via the UI; will not auto-start until re-enabled.'
                    : `Disabled in configuration (${stopReasons[t.key] || 'ENABLE_*'}); set the env var to true to enable.`;
                const action = (stoppedAndEnv || stoppedOnly)
                    ? `<button type="button" class="process-action-btn enable-btn" onclick="enableProcessType('${escapeAttr(t.key)}')" title="Clear the stopped flag; this type auto-starts on the next boot">Re-enable</button>`
                    : '';
                return `<div class="process-card disabled">
                    <div class="process-card-header">
                        <div class="process-card-header__main">
                            <div class="process-card-name" title="${escapeHtml(t.name)}">${escapeHtml(t.name)}</div>
                            <div class="process-card-meta">
                                <span class="category-badge">${escapeHtml(t.category)}</span>
                                ${badge}
                            </div>
                        </div>
                        ${action ? `<div class="process-card-actions">${action}</div>` : ''}
                    </div>
                    <div class="process-card-body">
                        <div class="process-card-message" title="${escapeHtml(reason)}">${escapeHtml(reason)}</div>
                    </div>
                </div>`;
            }).join('')}
        </div>`;
}

async function enableProcessType(key) {
    if (!confirm('Re-enable this process type? It will auto-start again on the next server boot.')) return;
    try {
        const res = await fetch(`/api/processes/types/${encodeURIComponent(key)}/enable`, { method: 'POST' });
        if (res.ok) {
            const data = await res.json();
            alert(data.message || 'Process type enabled');
        } else {
            const err = await res.text();
            alert('Failed to enable: ' + err);
        }
    } catch (e) {
        alert('Error: ' + e.message);
    }
    loadProcesses();
}

function escapeHtml(str) {
    return String(str ?? '')
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

function escapeAttr(str) {
    return escapeHtml(str).replace(/'/g, '&#39;');
}

async function triggerProcess(id) {
    if (!confirm('Trigger this process immediately?')) return;
    const btn = event.target;
    btn.disabled = true;
    btn.textContent = 'Running...';

    try {
        const res = await fetch(`/api/processes/${id}/trigger`, { method: 'POST' });
        if (res.ok) {
            const data = await res.json();
            alert(data.message || 'Process triggered');
            setTimeout(() => loadProcesses(), 500);
        } else {
            const err = await res.text();
            alert('Failed to trigger: ' + err);
        }
    } catch (e) {
        alert('Error: ' + e.message);
    } finally {
        btn.disabled = false;
        btn.textContent = '▶ Trigger';
    }
}

async function pauseProcess(id) {
    await fetch(`/api/processes/${id}/pause`, { method: 'POST' });
    loadProcesses();
}

async function resumeProcess(id) {
    await fetch(`/api/processes/${id}/resume`, { method: 'POST' });
    loadProcesses();
}

async function stopProcess(id) {
    if (!confirm('Stop this process?')) return;
    await fetch(`/api/processes/${id}/stop`, { method: 'POST' });
    loadProcesses();
}

function clearCompleted() {
    fetch('/api/processes/clear', { method: 'POST' })
        .then(() => loadProcesses())
        .catch(() => loadProcesses());
}

function refreshProcesses() {
    loadProcesses();
}

let processesInterval = null;
let currentSidebarProcessId = null;

document.addEventListener('DOMContentLoaded', () => {
    document.getElementById('processes-search')?.addEventListener('input', applyProcessFilters);
    document.getElementById('processes-filter')?.addEventListener('change', applyProcessFilters);
    document.getElementById('processes-category')?.addEventListener('change', applyProcessFilters);
    document.getElementById('processes-sort')?.addEventListener('change', applyProcessFilters);

    loadProcesses();
    processesInterval = setInterval(loadProcesses, 2000);
});

async function openProcessSidebar(processId) {
    currentSidebarProcessId = processId;
    const overlay = document.getElementById('process-sidebar-overlay');
    const sidebar = document.getElementById('process-sidebar');
    const title = document.getElementById('process-sidebar-title');
    const body = document.getElementById('process-sidebar-body');

    title.textContent = `Process #${processId}`;
    body.innerHTML = '<div class="empty-msg">Loading...</div>';
    overlay.classList.add('open');
    sidebar.classList.add('open');

    try {
        const [processRes, logsRes] = await Promise.all([
            fetch('/api/processes'),
            fetch(`/api/processes/${processId}/logs?limit=500`),
        ]);

        if (!processRes.ok) throw new Error('Failed to load process');
        const processData = await processRes.json();
        const process = processData.processes.find((p) => p.id === processId);

        let logs = [];
        if (logsRes.ok) {
            logs = await logsRes.json();
        }

        renderProcessSidebar(process, logs);
    } catch (e) {
        body.innerHTML = `<div class="empty-msg">Failed to load: ${escapeHtml(e.message)}</div>`;
    }
}

function renderProcessSidebar(process, logs) {
    const body = document.getElementById('process-sidebar-body');
    if (!process) {
        body.innerHTML = '<div class="empty-msg">Process not found</div>';
        return;
    }

    const formatDate = (ts) => {
        if (!ts) return '—';
        try {
            const d = new Date(ts);
            if (isNaN(d.getTime())) return ts;
            return d.toLocaleString();
        } catch (e) {
            return ts;
        }
    };

    const formatProgress = (p) => {
        if (p === null || p === undefined) return '—';
        return p.toFixed(1) + '%';
    };

    let html = '<div class="process-sidebar-section">';
    html += '<h4>Meta Information</h4>';
    html += '<table class="process-meta-table">';
    html += `<tr><td>ID</td><td>#${process.id}</td></tr>`;
    html += `<tr><td>Name</td><td>${escapeHtml(process.name)}</td></tr>`;
    html += `<tr><td>Category</td><td>${escapeHtml(process.category)}</td></tr>`;
    html += `<tr><td>Status</td><td><span class="status-badge ${process.status}">${process.status}</span></td></tr>`;
    html += `<tr><td>Progress</td><td>${formatProgress(process.progress)}</td></tr>`;
    html += `<tr><td>Message</td><td>${escapeHtml(process.message || '—')}</td></tr>`;
    html += `<tr><td>Started</td><td>${formatDate(process.started_at)}</td></tr>`;
    html += `<tr><td>Finished</td><td>${formatDate(process.finished_at)}</td></tr>`;
    html += `<tr><td>Paused</td><td>${process.paused ? 'Yes' : 'No'}</td></tr>`;
    html += '</table></div>';

    html += '<div class="process-sidebar-section">';
    html += '<h4>Logs</h4>';
    if (!logs || logs.length === 0) {
        html += '<div class="empty-msg">No logs found for this process</div>';
    } else {
        html +=
            '<table class="process-logs-table"><thead><tr><th style="width:160px">Timestamp</th><th style="width:60px">Level</th><th>Message</th></tr></thead><tbody>';
        logs.forEach((log) => {
            html += `<tr>
                <td class="log-timestamp">${escapeHtml(log.timestamp)}</td>
                <td class="log-level-${log.level.toLowerCase()}">${escapeHtml(log.level)}</td>
                <td class="log-message">${escapeHtml(log.message)}</td>
            </tr>`;
        });
        html += '</tbody></table>';
    }
    html += '</div>';

    body.innerHTML = html;
}

function closeProcessSidebar() {
    document.getElementById('process-sidebar-overlay').classList.remove('open');
    document.getElementById('process-sidebar').classList.remove('open');
    currentSidebarProcessId = null;
}

async function resyncFolders() {
    const statusEl = document.getElementById('resync-status');
    const input = document.getElementById('resync-input');
    const removeInput = document.getElementById('resync-remove-input');
    const paths = (input.value || '')
        .split('\n')
        .map((p) => p.trim())
        .filter(Boolean);
    const remove = (removeInput.value || '')
        .split('\n')
        .map((p) => p.trim())
        .filter(Boolean);
    if (!paths.length && !remove.length) {
        statusEl.textContent = 'Enter at least one folder path.';
        statusEl.className = 'resync-status err';
        return;
    }
    const body = {};
    if (paths.length) body.paths = paths;
    if (remove.length) body.remove = remove;
    statusEl.textContent = 'Starting re-sync…';
    statusEl.className = 'resync-status';
    try {
        const res = await fetch('/api/index', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body),
        });
        const data = await res.json().catch(() => ({}));
        if (!res.ok) throw new Error(data.message || `HTTP ${res.status}`);
        statusEl.textContent = data.message || 'Re-sync started';
        statusEl.className = 'resync-status ok';
        loadProcesses();
    } catch (e) {
        statusEl.textContent = e.message;
        statusEl.className = 'resync-status err';
    }
}

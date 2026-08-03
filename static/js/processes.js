async function loadProcesses() {
    try {
        const res = await fetch('/api/processes');
        if (!res.ok) throw new Error('Failed to load processes');
        const data = await res.json();
        renderProcesses(data.processes || []);
    } catch (e) {
        document.getElementById('processes-tbody').innerHTML =
            `<tr><td colspan="8" class="empty-msg">Failed to load: ${escapeHtml(e.message)}</td></tr>`;
    }
}

function renderProcesses(processes) {
    const tbody = document.getElementById('processes-tbody');
    const summary = document.getElementById('processes-summary');

    const running = processes.filter(p => p.status === 'running').length;
    const pending = processes.filter(p => p.status === 'pending').length;
    const completed = processes.filter(p => p.status === 'completed').length;
    const failed = processes.filter(p => p.status === 'failed').length;

    summary.textContent = `${processes.length} total • ${running} running • ${pending} upcoming • ${completed} completed • ${failed} failed`;

    if (processes.length === 0) {
        tbody.innerHTML = '<tr><td colspan="8" class="empty-msg">No processes tracked yet</td></tr>';
        return;
    }

    tbody.innerHTML = processes.map(p => {
        const progressHtml = p.progress !== null && p.progress !== undefined
            ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'running' ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
            : '<span class="process-mono">—</span>';

        return `<tr>
            <td class="process-mono">#${p.id}</td>
            <td><strong>${escapeHtml(p.name)}</strong></td>
            <td><span class="category-badge">${escapeHtml(p.category)}</span></td>
            <td><span class="status-badge ${p.status}">${p.status}</span></td>
            <td style="min-width:120px">${progressHtml}</td>
            <td style="max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title="${escapeHtml(p.message || '')}">${escapeHtml(p.message || '—')}</td>
            <td class="process-mono">${formatDate(p.started_at)}</td>
            <td class="process-mono">${formatDate(p.finished_at)}</td>
        </tr>`;
    }).join('');
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

function clearCompleted() {
    fetch('/api/processes/clear', { method: 'POST' })
        .then(() => loadProcesses())
        .catch(() => loadProcesses());
}

function refreshProcesses() {
    loadProcesses();
}

let processesInterval = null;
document.addEventListener('DOMContentLoaded', () => {
    loadProcesses();
    processesInterval = setInterval(loadProcesses, 2000);
});

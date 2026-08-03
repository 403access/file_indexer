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
    const cardsContainer = document.getElementById('process-cards');

    const running = processes.filter(p => p.status === 'active').length;
    const pending = processes.filter(p => p.status === 'pending').length;
    const completed = processes.filter(p => p.status === 'completed').length;
    const failed = processes.filter(p => p.status === 'failed').length;

    summary.textContent = `${processes.length} total • ${running} running • ${pending} upcoming • ${completed} completed • ${failed} failed`;

    const active = processes.filter(p => p.status === 'active' || p.status === 'pending');

    if (active.length === 0) {
        cardsContainer.innerHTML = '';
    } else {
        cardsContainer.innerHTML = active.map(p => {
            const progressHtml = p.progress !== null && p.progress !== undefined
                ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'active' && !p.paused ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
                : '<span class="process-mono">—</span>';

            return `<div class="process-card ${p.status}${p.paused ? ' paused' : ''}">
                <div class="process-card-header">
                    <div>
                        <div class="process-card-name">${escapeHtml(p.name)}</div>
                        <div class="process-card-meta">
                            <span class="category-badge">${escapeHtml(p.category)}</span>
                            <span class="status-badge ${p.status}">${p.status}${p.paused ? ' (paused)' : ''}</span>
                        </div>
                    </div>
                    <div class="process-card-actions">
                        <button class="process-action-btn trigger-btn" onclick="triggerProcess(${p.id})" title="Run this process immediately">▶ Trigger</button>
                        ${p.paused
                            ? `<button class="process-action-btn resume-btn" onclick="resumeProcess(${p.id})" title="Resume the paused process">▶ Resume</button>`
                            : `<button class="process-action-btn pause-btn" onclick="pauseProcess(${p.id})" title="Temporarily suspend; you can resume later">⏸ Pause</button>`
                        }
                        <button class="process-action-btn stop-btn" onclick="stopProcess(${p.id})" title="Permanently terminate this process">⏹ Stop</button>
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
        }).join('');
    }

    if (processes.length === 0) {
        tbody.innerHTML = '<tr><td colspan="8" class="empty-msg">No processes tracked yet</td></tr>';
        return;
    }

    tbody.innerHTML = processes.map(p => {
        const progressHtml = p.progress !== null && p.progress !== undefined
            ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'active' && !p.paused ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
            : '<span class="process-mono">—</span>';

        return `<tr>
            <td class="process-mono">#${p.id}</td>
            <td><strong>${escapeHtml(p.name)}</strong></td>
            <td><span class="category-badge">${escapeHtml(p.category)}</span></td>
            <td><span class="status-badge ${p.status}">${p.status}${p.paused ? ' (paused)' : ''}</span></td>
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
            fetch(`/api/processes/${processId}/logs?limit=500`)
        ]);

        if (!processRes.ok) throw new Error('Failed to load process');
        const processData = await processRes.json();
        const process = processData.processes.find(p => p.id === processId);

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
        html += '<table class="process-logs-table"><thead><tr><th style="width:160px">Timestamp</th><th style="width:60px">Level</th><th>Message</th></tr></thead><tbody>';
        logs.forEach(log => {
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
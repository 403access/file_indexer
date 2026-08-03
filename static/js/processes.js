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

    const running = processes.filter(p => p.status === 'running').length;
    const pending = processes.filter(p => p.status === 'pending').length;
    const completed = processes.filter(p => p.status === 'completed').length;
    const failed = processes.filter(p => p.status === 'failed').length;

    summary.textContent = `${processes.length} total • ${running} running • ${pending} upcoming • ${completed} completed • ${failed} failed`;

    const active = processes.filter(p => p.status === 'running' || p.status === 'pending');

    if (active.length === 0) {
        cardsContainer.innerHTML = '';
    } else {
        cardsContainer.innerHTML = active.map(p => {
            const progressHtml = p.progress !== null && p.progress !== undefined
                ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'running' && !p.paused ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
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
            ? `<div class="progress-bar-bg"><div class="progress-bar-fill${p.status === 'running' && !p.paused ? ' active' : ''}" style="width:${Math.min(100, Math.max(0, p.progress))}%"></div></div><div class="process-mono">${p.progress.toFixed(0)}%</div>`
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
let currentProcessLogsProcessId = null;

document.addEventListener('DOMContentLoaded', () => {
    loadProcesses();
    processesInterval = setInterval(loadProcesses, 2000);
});

async function showProcessLogs(processId, processName) {
    currentProcessLogsProcessId = processId;
    const overlay = document.getElementById('process-logs-overlay');
    const title = document.getElementById('process-logs-title');
    const body = document.getElementById('process-logs-body');

    title.textContent = `Logs: ${processName} (#${processId})`;
    body.innerHTML = '<div class="empty-msg">Loading logs...</div>';
    overlay.style.display = 'block';

    try {
        const res = await fetch(`/api/processes/${processId}/logs?limit=500`);
        if (!res.ok) throw new Error('Failed to load logs');
        const logs = await res.json();
        renderProcessLogs(logs);
    } catch (e) {
        body.innerHTML = `<div class="empty-msg">Failed to load logs: ${escapeHtml(e.message)}</div>`;
    }
}

function renderProcessLogs(logs) {
    const body = document.getElementById('process-logs-body');
    if (!logs || logs.length === 0) {
        body.innerHTML = '<div class="empty-msg">No logs found for this process</div>';
        return;
    }

    let html = '<table class="process-logs-table"><thead><tr><th style="width:180px">Timestamp</th><th style="width:60px">Level</th><th>Message</th></tr></thead><tbody>';
    logs.forEach(log => {
        html += `<tr>
            <td class="log-timestamp">${escapeHtml(log.timestamp)}</td>
            <td class="log-level-${log.level.toLowerCase()}">${escapeHtml(log.level)}</td>
            <td class="log-message">${escapeHtml(log.message)}</td>
        </tr>`;
    });
    html += '</tbody></table>';
    body.innerHTML = html;
}

function closeProcessLogs() {
    document.getElementById('process-logs-overlay').style.display = 'none';
    currentProcessLogsProcessId = null;
}
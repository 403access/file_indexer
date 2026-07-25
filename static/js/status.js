let statusPollInterval = null;

async function pollStatus() {
    try {
        const res = await fetch('/api/status');
        const data = await res.json();
        const dot = document.getElementById('status-dot');
        if (!dot) return;

        if (data.status === 'indexing') {
            dot.className = 'status-dot indexing';
            dot.title = `Indexing: ${data.current_dir || '...'} (${data.total_entries} entries)`;
        } else {
            dot.className = 'status-dot idle';
            dot.title = 'Idle';
            if (statusPollInterval) {
                clearInterval(statusPollInterval);
                statusPollInterval = null;
            }
        }
    } catch (e) {
        const dot = document.getElementById('status-dot');
        if (dot) {
            dot.className = 'status-dot error';
            dot.title = 'Could not reach server';
        }
    }
}

function startStatusPolling() {
    pollStatus();
    statusPollInterval = setInterval(pollStatus, 2000);
}

document.addEventListener('DOMContentLoaded', startStatusPolling);

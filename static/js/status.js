let statusPollTimeout = null;
let statusPolling = false;

async function pollStatus() {
    if (statusPolling) return;
    statusPolling = true;

    try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 10000);

        const res = await fetch('/api/status', { signal: controller.signal });
        clearTimeout(timeoutId);
        const data = await res.json();
        const dot = document.getElementById('status-dot');
        if (!dot) return;

        if (data.status === 'indexing') {
            dot.className = 'status-dot indexing';
            dot.title = `Indexing: ${data.current_dir || '...'} (${data.total_entries} entries)`;
            scheduleNext(2000);
        } else {
            dot.className = 'status-dot idle';
            dot.title = 'Idle';
        }
    } catch (e) {
        const dot = document.getElementById('status-dot');
        if (dot) {
            dot.className = 'status-dot error';
            dot.title = 'Could not reach server';
        }
        scheduleNext(10000);
    } finally {
        statusPolling = false;
    }
}

function scheduleNext(ms) {
    if (statusPollTimeout) clearTimeout(statusPollTimeout);
    statusPollTimeout = setTimeout(pollStatus, ms);
}

function startStatusPolling() {
    pollStatus();
}

document.addEventListener('DOMContentLoaded', startStatusPolling);

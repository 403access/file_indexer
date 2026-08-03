const FILE_TYPE_TEMPLATES = {
    photo_album: { label: 'Photo album', types: ['jpg','jpeg','png','gif','bmp','webp','tiff','tif','heic','heif','raw','cr2','nef','arw','dng'] },
    documents: { label: 'Documents', types: ['pdf','doc','docx','xls','xlsx','ppt','pptx','txt','rtf','odt','ods','csv'] },
    video: { label: 'Video', types: ['mp4','mkv','avi','mov','wmv','flv','webm','m4v','mpg','mpeg'] },
    audio: { label: 'Audio', types: ['mp3','wav','flac','aac','ogg','m4a','wma','opus'] },
    code: { label: 'Code', types: ['js','ts','jsx','tsx','py','rb','go','rs','java','c','cpp','h','hpp','cs','php','html','css','scss','json','xml','yaml','yml','toml','sh','md','sql'] },
    config: { label: 'Config', types: ['json','yaml','yml','toml','ini','cfg','conf','env','xml'] },
};

let availableFileTypes = [];
let selectedFileTypes = new Set();
let activeTemplate = null;

async function loadAvailableFileTypes() {
    try {
        const res = await fetch('/api/duplicate-folders/types');
        if (res.ok) {
            const data = await res.json();
            availableFileTypes = data.types || [];
        }
    } catch (e) {
        console.error('Failed to load file types:', e);
    }
}

function renderFileTypeList() {
    const list = document.getElementById('ft-list');
    if (availableFileTypes.length === 0) {
        list.innerHTML = '<div class="ft-empty">No file types found</div>';
        return;
    }

    let html = '';
    if (activeTemplate) {
        const tpl = FILE_TYPE_TEMPLATES[activeTemplate];
        const templateTypes = availableFileTypes.filter(t => tpl.types.includes(t));
        const otherTypes = availableFileTypes.filter(t => !tpl.types.includes(t));

        if (templateTypes.length > 0) {
            html += `<div class="ft-group-label">${tpl.label}</div>`;
            html += templateTypes.map(t => `
                <label class="ft-item">
                    <input type="checkbox" value="${t}" ${selectedFileTypes.has(t) ? 'checked' : ''} onchange="toggleFileType(this)">
                    <span>${t}</span>
                </label>
            `).join('');
        }

        if (otherTypes.length > 0) {
            html += `<div class="ft-group-label ft-group-label-other">Other types</div>`;
            html += otherTypes.map(t => `
                <label class="ft-item">
                    <input type="checkbox" value="${t}" ${selectedFileTypes.has(t) ? 'checked' : ''} onchange="toggleFileType(this)">
                    <span>${t}</span>
                </label>
            `).join('');
        }
    } else {
        html = availableFileTypes.map(t => `
            <label class="ft-item">
                <input type="checkbox" value="${t}" ${selectedFileTypes.has(t) ? 'checked' : ''} onchange="toggleFileType(this)">
                <span>${t}</span>
            </label>
        `).join('');
    }

    list.innerHTML = html;
    updateFileTypeCount();
}

function toggleFileType(cb) {
    if (cb.checked) selectedFileTypes.add(cb.value);
    else selectedFileTypes.delete(cb.value);
    applyFilter();
    updateFileTypeCount();
}

function updateFileTypeCount() {
    document.getElementById('ft-count').textContent =
        selectedFileTypes.size ? ` (${selectedFileTypes.size})` : '';
}

function selectAllFileTypes() {
    availableFileTypes.forEach(t => selectedFileTypes.add(t));
    renderFileTypeList();
    applyFilter();
}

function unselectAllFileTypes() {
    selectedFileTypes.clear();
    renderFileTypeList();
    applyFilter();
}

function applyFileTypeTemplate() {
    const sel = document.getElementById('ft-template').value;
    if (!sel) {
        activeTemplate = null;
        renderFileTypeList();
        return;
    }
    const tpl = FILE_TYPE_TEMPLATES[sel];
    if (!tpl) return;
    activeTemplate = sel;
    selectedFileTypes = new Set(tpl.types.filter(t => availableFileTypes.includes(t)));
    renderFileTypeList();
    applyFilter();
}

function toggleFileTypePanel() {
    const panel = document.getElementById('ft-panel');
    if (panel.style.display === 'none') {
        panel.style.display = 'flex';
        if (availableFileTypes.length === 0) {
            loadAvailableFileTypes().then(renderFileTypeList);
        } else {
            renderFileTypeList();
        }
    } else {
        panel.style.display = 'none';
    }
}

function closeFileTypePanel() {
    document.getElementById('ft-panel').style.display = 'none';
}

// File Viewer Sidebar Component
(function() {
    const VIEWER_TYPES = {
        text: { label: 'Text', extensions: ['txt','log','csv','tsv','conf','rs','py','js','ts','jsx','tsx','java','c','cpp','h','hpp','go','rb','php','swift','kt','scala','sh','bash','zsh','fish','ps1','bat','cmd','sql','r','lua','perl','pl','html','htm','css','scss','less','xml','json','yaml','yml','toml','ini','cfg','md','rst','tex','vue','svelte'] },
        image: { label: 'Image', extensions: ['png','jpg','jpeg','gif','svg','webp','bmp','ico'] },
        pdf: { label: 'PDF', extensions: ['pdf'] },
        binary: { label: 'Binary', extensions: [] }
    };

    function detectType(filename) {
        const ext = filename.split('.').pop().toLowerCase();
        for (const [type, config] of Object.entries(VIEWER_TYPES)) {
            if (config.extensions.includes(ext)) return type;
        }
        return null;
    }

    function formatSize(bytes) {
        if (bytes === 0) return '0 B';
        const units = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(1024));
        return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i];
    }

    function formatDate(ts) {
        if (!ts) return '-';
        return new Date(ts * 1000).toLocaleString();
    }

    function escapeHtml(str) {
        const div = document.createElement('div');
        div.textContent = str;
        return div.innerHTML;
    }

    function loadStylesheet() {
        if (document.getElementById('file-viewer-css')) return;
        const link = document.createElement('link');
        link.id = 'file-viewer-css';
        link.rel = 'stylesheet';
        link.href = '/css/file-viewer.css';
        document.head.appendChild(link);
    }

    function createSidebarHTML() {
        if (document.getElementById('file-viewer-overlay')) return;

        const overlay = document.createElement('div');
        overlay.id = 'file-viewer-overlay';
        overlay.className = 'file-viewer-overlay';
        overlay.onclick = () => window.FileViewer.close();

        const sidebar = document.createElement('div');
        sidebar.id = 'file-viewer-sidebar';
        sidebar.className = 'file-viewer-sidebar';
        sidebar.innerHTML = `
            <div class="fv-header">
                <button class="fv-close" onclick="window.FileViewer.close()">&times;</button>
                <span class="fv-filename" id="fv-filename"></span>
                <div class="fv-type-selector" id="fv-type-selector"></div>
            </div>
            <dl class="fv-meta" id="fv-meta"></dl>
            <div class="fv-content" id="fv-content"></div>
        `;

        document.body.appendChild(overlay);
        document.body.appendChild(sidebar);
    }

    function renderMeta(file) {
        const meta = document.getElementById('fv-meta');
        meta.innerHTML = `
            <dt>Name</dt><dd>${escapeHtml(file.name)}</dd>
            <dt>Size</dt><dd>${formatSize(file.size)}</dd>
            <dt>Modified</dt><dd>${formatDate(file.modified)}</dd>
            ${file.hash ? `<dt>Hash</dt><dd>${escapeHtml(file.hash.substring(0, 16))}...</dd>` : ''}
            <dt>Path</dt><dd style="font-size:0.75rem;color:#666">${escapeHtml(file.path || '')}</dd>
        `;
    }

    function renderTypeSelector(currentType) {
        const container = document.getElementById('fv-type-selector');
        const types = ['text', 'image'];
        container.innerHTML = types.map(type =>
            `<button class="fv-type-btn ${type === currentType ? 'active' : ''}"
                     onclick="window.FileViewer.setViewType('${type}')">${VIEWER_TYPES[type].label}</button>`
        ).join('');
    }

    function renderContent(content, type, isText) {
        const container = document.getElementById('fv-content');

        if (type === 'text' || isText) {
            container.innerHTML = `<pre class="fv-text">${escapeHtml(content)}</pre>`;
        } else if (type === 'image') {
            const blob = new Blob([content]);
            const url = URL.createObjectURL(blob);
            container.innerHTML = `<div class="fv-image"><img src="${url}" alt="File preview"></div>`;
        } else {
            container.innerHTML = `
                <div class="fv-binary">
                    <div class="fv-binary-icon">\uD83D\uDCC4</div>
                    <div>Binary file</div>
                    <div style="font-size:0.8rem">${formatSize(content.byteLength || 0)}</div>
                </div>
            `;
        }
    }

    async function openFile(filePath, fileName) {
        loadStylesheet();
        createSidebarHTML();

        const overlay = document.getElementById('file-viewer-overlay');
        const sidebar = document.getElementById('file-viewer-sidebar');
        const filenameEl = document.getElementById('fv-filename');
        const contentEl = document.getElementById('fv-content');

        filenameEl.textContent = fileName;
        contentEl.innerHTML = '<div class="fv-loading">Loading...</div>';

        overlay.classList.add('open');
        sidebar.classList.add('open');

        try {
            const normalizedPath = filePath.replace(/\/+/g, '/');
            const response = await fetch(`/api/file?path=${encodeURIComponent(normalizedPath)}`);
            if (!response.ok) throw new Error('Failed to load file');

            const isText = response.headers.get('x-is-text') === 'true';
            const fileSize = parseInt(response.headers.get('x-file-size') || '0');

            let detectedType = detectType(fileName);
            if (!detectedType) {
                detectedType = isText ? 'text' : 'binary';
            }

            window.FileViewer._currentType = detectedType;
            window.FileViewer._currentPath = filePath;
            window.FileViewer._currentFileName = fileName;

            renderMeta({
                name: fileName,
                size: fileSize,
                modified: null,
                hash: null,
                path: filePath
            });

            renderTypeSelector(detectedType);

            if (detectedType === 'image' && !isText) {
                const blob = await response.blob();
                renderContent(blob, 'image', false);
            } else {
                const text = await response.text();
                renderContent(text, detectedType, isText);
            }
        } catch (err) {
            contentEl.innerHTML = `<div class="fv-error">Error: ${escapeHtml(err.message)}</div>`;
        }
    }

    function close() {
        const overlay = document.getElementById('file-viewer-overlay');
        const sidebar = document.getElementById('file-viewer-sidebar');
        if (overlay) overlay.classList.remove('open');
        if (sidebar) {
            sidebar.classList.remove('open');
            setTimeout(() => {
                overlay?.remove();
                sidebar?.remove();
            }, 300);
        }
    }

    function setViewType(type) {
        const path = window.FileViewer._currentPath;
        const name = window.FileViewer._currentFileName;
        if (path && name) {
            window.FileViewer._currentType = type;
            renderTypeSelector(type);
            openFile(path, name);
        }
    }

    window.FileViewer = {
        open: openFile,
        close,
        setViewType,
        _currentType: null,
        _currentPath: null,
        _currentFileName: null
    };
})();

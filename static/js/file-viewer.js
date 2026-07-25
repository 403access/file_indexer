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

    function createSidebarStyles() {
        if (document.getElementById('file-viewer-styles')) return;
        const style = document.createElement('style');
        style.id = 'file-viewer-styles';
        style.textContent = `
            .file-viewer-overlay {
                position: fixed;
                top: 0;
                left: 0;
                right: 0;
                bottom: 0;
                background: rgba(0,0,0,0.3);
                z-index: 999;
                opacity: 0;
                transition: opacity 0.2s;
            }
            .file-viewer-overlay.open { opacity: 1; }
            .file-viewer-sidebar {
                position: fixed;
                top: 0;
                right: 0;
                width: 50vw;
                max-width: 800px;
                min-width: 400px;
                height: 100vh;
                background: white;
                box-shadow: -4px 0 20px rgba(0,0,0,0.15);
                z-index: 1000;
                display: flex;
                flex-direction: column;
                transform: translateX(100%);
                transition: transform 0.25s ease;
            }
            .file-viewer-sidebar.open { transform: translateX(0); }
            .fv-header {
                display: flex;
                align-items: center;
                gap: 0.75rem;
                padding: 1rem 1.25rem;
                border-bottom: 1px solid #eee;
                background: #f8f9fa;
            }
            .fv-close {
                background: none;
                border: none;
                font-size: 1.2rem;
                cursor: pointer;
                color: #666;
                padding: 0.25rem;
                border-radius: 4px;
            }
            .fv-close:hover { background: #e0e0e0; }
            .fv-filename {
                flex: 1;
                font-weight: 600;
                font-size: 0.95rem;
                overflow: hidden;
                text-overflow: ellipsis;
                white-space: nowrap;
            }
            .fv-type-selector {
                display: flex;
                gap: 0.25rem;
            }
            .fv-type-btn {
                padding: 0.25rem 0.5rem;
                border: 1px solid #ddd;
                border-radius: 4px;
                background: white;
                font-size: 0.75rem;
                cursor: pointer;
                color: #666;
            }
            .fv-type-btn.active {
                background: #3498db;
                color: white;
                border-color: #3498db;
            }
            .fv-type-btn:hover:not(.active) {
                background: #f0f0f0;
            }
            .fv-meta {
                padding: 0.75rem 1.25rem;
                border-bottom: 1px solid #eee;
                display: grid;
                grid-template-columns: auto 1fr;
                gap: 0.3rem 1rem;
                font-size: 0.8rem;
            }
            .fv-meta dt {
                color: #999;
                font-weight: 500;
            }
            .fv-meta dd {
                color: #333;
                font-family: monospace;
                word-break: break-all;
            }
            .fv-content {
                flex: 1;
                overflow: auto;
                padding: 0;
            }
            .fv-text {
                padding: 1rem 1.25rem;
                font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
                font-size: 0.8rem;
                line-height: 1.5;
                white-space: pre-wrap;
                word-break: break-all;
                color: #333;
                margin: 0;
                background: #fafbfc;
            }
            .fv-image {
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 1rem;
                min-height: 100%;
                background: #f5f5f5;
            }
            .fv-image img {
                max-width: 100%;
                max-height: 100%;
                object-fit: contain;
                border-radius: 4px;
                box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            }
            .fv-binary {
                display: flex;
                flex-direction: column;
                align-items: center;
                justify-content: center;
                padding: 3rem;
                color: #999;
                text-align: center;
                gap: 0.5rem;
            }
            .fv-binary-icon { font-size: 3rem; }
            .fv-loading {
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 3rem;
                color: #999;
            }
            .fv-error {
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 3rem;
                color: #e74c3c;
            }
        `;
        document.head.appendChild(style);
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
        createSidebarStyles();
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

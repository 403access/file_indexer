// File Viewer Sidebar Component
(function() {
    const VIEWER_TYPES = {
        text: { label: 'Text', extensions: ['txt','log','csv','tsv','conf','rs','py','js','ts','jsx','tsx','java','c','cpp','h','hpp','go','rb','php','swift','kt','scala','sh','bash','zsh','fish','ps1','bat','cmd','sql','r','lua','perl','pl','html','htm','css','scss','less','xml','json','yaml','yml','toml','ini','cfg','md','rst','tex','vue','svelte'] },
        image: { label: 'Image', extensions: ['png','jpg','jpeg','gif','svg','webp','bmp','ico'] },
        pdf: { label: 'PDF', extensions: ['pdf'] },
        binary: { label: 'Binary', extensions: [] }
    };

    let _cwd = null;
    let _cwdPromise = null;

    function fetchCwd() {
        if (_cwdPromise) return _cwdPromise;
        _cwdPromise = fetch('/api/config').then(r => r.json()).then(d => {
            _cwd = d.cwd.replace(/\/+$/, '');
            return _cwd;
        }).catch(() => {
            _cwd = '';
            return '';
        });
        return _cwdPromise;
    }

    function toRelativePath(absPath, cwd) {
        if (!cwd) return absPath;
        const normalCwd = cwd.replace(/\/+$/, '');
        if (absPath.startsWith(normalCwd + '/')) {
            return absPath.slice(normalCwd.length + 1);
        }
        if (absPath === normalCwd) return '.';
        return absPath;
    }

    function toAbsolutePath(relPath, cwd) {
        if (!cwd) return relPath;
        if (relPath.startsWith('/')) return relPath;
        return cwd + '/' + relPath;
    }

    function resolveDisplayPath(absPath) {
        if (!_cwdPromise) fetchCwd();
        if (window.FileViewer._pathMode === 'relative' && _cwd) {
            return toRelativePath(absPath, _cwd);
        }
        return absPath;
    }

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

    function parentDir(path) {
        const parts = path.replace(/\/+/g, '/').split('/').filter(Boolean);
        parts.pop();
        return parts.length ? '/' + parts.join('/') : '/';
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
                <button class="fv-path-toggle" id="fv-path-toggle" onclick="window.FileViewer.togglePathMode()" title="Toggle absolute/relative path"></button>
                <div class="fv-type-selector" id="fv-type-selector"></div>
            </div>
            <dl class="fv-meta" id="fv-meta"></dl>
            <div class="fv-body">
                <div class="fv-tree" id="fv-tree"></div>
                <div class="fv-content" id="fv-content"></div>
            </div>
        `;

        document.body.appendChild(overlay);
        document.body.appendChild(sidebar);
    }

    function buildPathTree(displayPath) {
        const parts = displayPath.replace(/\/+/g, '/').split('/').filter(Boolean);
        if (parts.length === 0) return '<div class="fv-tree-line current">/</div>';
        let lines = [];
        for (let i = 0; i < parts.length; i++) {
            const isLast = i === parts.length - 1;
            const indent = '  '.repeat(i);
            const connector = isLast ? '\u2514\u2500 ' : '\u251C\u2500 ';
            const icon = isLast ? '\uD83D\uDCC4' : '\uD83D\uDCC1';
            lines.push(`<div class="fv-tree-line${isLast ? ' current' : ''}"><span class="fv-tree-prefix">${indent}${connector}</span>${icon} ${parts[i]}</div>`);
        }
        return lines.join('');
    }

    function renderMeta(file) {
        const meta = document.getElementById('fv-meta');
        const displayPath = resolveDisplayPath(file.path);
        const displayParent = file.parent ? resolveDisplayPath(file.parent) : '';
        meta.innerHTML = `
            <dt>Name</dt><dd>${escapeHtml(file.name)}</dd>
            <dt>Type</dt><dd>${file.ext ? escapeHtml(file.ext.toUpperCase()) : '(none)'} <span style="color:#999;font-size:0.7rem">${escapeHtml(file.mime)}</span></dd>
            <dt>Size</dt><dd>${formatSize(file.size)}</dd>
            <dt>Path</dt><dd class="fv-meta-path" style="font-size:0.75rem;word-break:break-all">${escapeHtml(displayPath)}</dd>
            <dt>Directory</dt><dd class="fv-meta-parent" style="font-size:0.75rem">${escapeHtml(displayParent)}</dd>
            <dt>Modified</dt><dd>${formatDate(file.modified)}</dd>
            <dt>Created</dt><dd>${formatDate(file.created)}</dd>
            <dt>Permissions</dt><dd>${escapeHtml(file.permissions)}</dd>
            <dt>Viewer</dt><dd>${escapeHtml(file.type)}</dd>
        `;
        const tree = document.getElementById('fv-tree');
        if (tree && file.path) {
            tree.innerHTML = buildPathTree(displayPath);
        }
        updatePathToggleLabel();
    }

    function updatePathToggleLabel() {
        const btn = document.getElementById('fv-path-toggle');
        if (!btn) return;
        const mode = window.FileViewer._pathMode;
        btn.textContent = mode === 'relative' ? 'Rel' : 'Abs';
        btn.title = mode === 'relative' ? 'Showing relative path — click for absolute' : 'Showing absolute path — click for relative';
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
        await fetchCwd();

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
            const ext = response.headers.get('x-ext') || '';
            const mime = response.headers.get('x-mime') || '';
            const isDir = response.headers.get('x-is-dir') === 'true';
            const permissions = response.headers.get('x-permissions') || '';
            const modifiedTs = parseInt(response.headers.get('x-modified') || '0');
            const createdTs = parseInt(response.headers.get('x-created') || '0');
            const parent = response.headers.get('x-parent') || parentDir(filePath);

            let detectedType = detectType(fileName);
            if (!detectedType) {
                detectedType = isText ? 'text' : 'binary';
            }

            window.FileViewer._currentType = detectedType;
            window.FileViewer._currentPath = filePath;
            window.FileViewer._currentFileName = fileName;
            window.FileViewer._currentFileData = {
                name: fileName,
                size: fileSize,
                modified: modifiedTs || null,
                hash: null,
                path: filePath,
                ext: ext,
                mime: mime,
                isDir: isDir,
                permissions: permissions,
                created: createdTs || null,
                parent: parent,
                type: detectedType
            };

            renderMeta(window.FileViewer._currentFileData);
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

    function togglePathMode() {
        window.FileViewer._pathMode = window.FileViewer._pathMode === 'relative' ? 'absolute' : 'relative';
        const data = window.FileViewer._currentFileData;
        if (data) renderMeta(data);
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
        togglePathMode,
        _currentType: null,
        _currentPath: null,
        _currentFileName: null,
        _currentFileData: null,
        _pathMode: 'absolute'
    };
})();

const API = {
    async search(params) {
        const query = new URLSearchParams();
        if (params.name) query.set('name', params.name);
        if (params.type) query.set('type', params.type);
        if (params.pattern) query.set('pattern', params.pattern);
        if (params.sort) query.set('sort', params.sort);
        if (params.order) query.set('order', params.order);
        if (params.page) query.set('page', params.page);
        if (params.per_page) query.set('per_page', params.per_page);

        const res = await fetch(`/api/search?${query}`);
        if (!res.ok) throw new Error(`Search failed: ${res.statusText}`);
        return res.json();
    }
};

function formatSize(bytes) {
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + units[i];
}

function formatDate(timestamp) {
    if (!timestamp) return '-';
    return new Date(timestamp * 1000).toLocaleDateString();
}

let currentPage = 1;
const perPage = 20;
let currentParams = {};

async function doSearch(page = 1) {
    currentParams = {
        name: document.getElementById('search-name').value,
        type: document.getElementById('search-type').value,
        pattern: document.getElementById('search-pattern').value,
        sort: document.getElementById('search-sort').value,
        order: document.getElementById('search-order').value,
        page: page,
        per_page: perPage
    };

    try {
        const data = await API.search(currentParams);
        currentPage = data.page;
        renderResults(data);
    } catch (err) {
        console.error(err);
    }
}

function renderResults(data) {
    const table = document.getElementById('results-table');
    const tbody = document.getElementById('results-body');
    const noResults = document.getElementById('no-results');
    const info = document.getElementById('results-info');
    const count = document.getElementById('result-count');
    const pagination = document.getElementById('pagination');

    if (data.results.length === 0) {
        table.style.display = 'none';
        noResults.style.display = 'block';
        info.style.display = 'none';
        pagination.style.display = 'none';
        return;
    }

    noResults.style.display = 'none';
    table.style.display = 'table';
    info.style.display = 'block';
    pagination.style.display = 'flex';

    count.textContent = `${data.total} result${data.total !== 1 ? 's' : ''}`;

    tbody.innerHTML = data.results.map(entry => `
        <tr>
            <td class="name">${entry.name}</td>
            <td class="path" title="${entry.path || ''}">${truncatePath(entry.path)}</td>
            <td class="size">${entry.is_directory ? '-' : formatSize(entry.size)}</td>
            <td class="modified">${formatDate(entry.modified)}</td>
            <td class="type">${entry.is_directory ? 'Dir' : 'File'}</td>
        </tr>
    `).join('');

    const totalPages = Math.ceil(data.total / perPage);
    document.getElementById('page-info').textContent = `Page ${data.page} of ${totalPages}`;
    document.getElementById('prev-page').disabled = data.page <= 1;
    document.getElementById('next-page').disabled = data.page >= totalPages;
}

function truncatePath(path) {
    if (!path) return '';
    const parts = path.split('/');
    if (parts.length <= 3) return path;
    return '.../' + parts.slice(-2).join('/');
}

document.getElementById('search-form').addEventListener('submit', (e) => {
    e.preventDefault();
    doSearch(1);
});

document.getElementById('prev-page').addEventListener('click', () => {
    if (currentPage > 1) doSearch(currentPage - 1);
});

document.getElementById('next-page').addEventListener('click', () => {
    doSearch(currentPage + 1);
});

document.getElementById('search-sort').addEventListener('change', () => doSearch(1));
document.getElementById('search-order').addEventListener('change', () => doSearch(1));

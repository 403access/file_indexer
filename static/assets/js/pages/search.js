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
    const wrap = document.getElementById('results-wrap');
    const tbody = document.getElementById('results-body');
    const noResults = document.getElementById('no-results');
    const info = document.getElementById('results-info');
    const count = document.getElementById('result-count');
    const pagination = document.getElementById('pagination');

    if (data.results.length === 0) {
        table.style.display = 'none';
        if (wrap) wrap.style.display = 'none';
        noResults.style.display = 'block';
        info.style.display = 'none';
        pagination.style.display = 'none';
        return;
    }

    noResults.style.display = 'none';
    table.style.display = 'table';
    if (wrap) wrap.style.display = '';
    info.style.display = 'block';
    pagination.style.display = 'flex';

    count.textContent = `${data.total} result${data.total !== 1 ? 's' : ''}`;

    tbody.innerHTML = data.results.map(entry => `
        <tr class="${entry.is_directory ? 'clickable' : ''}" onclick="${entry.is_directory ? `openFolder('${entry.path.replace(/'/g, "\\'")}')` : `FileViewer.open('${entry.path.replace(/'/g, "\\'")}', '${entry.name.replace(/'/g, "\\'")}')`}">
            <td class="name">${entry.is_directory ? '\uD83D\uDCC1 ' : '\uD83D\uDCC4 '}${entry.name}</td>
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

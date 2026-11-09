// ProcureAI Client-side Logic

const API = {
  getToken() {
    return localStorage.getItem('procure_token');
  },
  setToken(token) {
    localStorage.setItem('procure_token', token);
  },
  clearToken() {
    localStorage.removeItem('procure_token');
    localStorage.removeItem('procure_user');
  },
  getUser() {
    try {
      return JSON.parse(localStorage.getItem('procure_user'));
    } catch {
      return null;
    }
  },
  setUser(user) {
    localStorage.setItem('procure_user', JSON.stringify(user));
  },
  async request(endpoint, options = {}) {
    const token = this.getToken();
    const headers = {
      'Content-Type': 'application/json',
      ...options.headers,
    };
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

    const res = await fetch(endpoint, {
      ...options,
      headers,
    });

    if (res.status === 401 && !window.location.pathname.endsWith('login.html')) {
      this.clearToken();
      window.location.href = '/login.html';
      throw new Error('Unauthorized');
    }

    if (options.rawResponse) {
      return res;
    }

    const data = await res.json();
    if (!res.ok) {
      const msg = data.error?.message || data.message || `HTTP ${res.status}`;
      throw new Error(msg);
    }
    return data;
  }
};

// Authentication Check on Page Load
function checkAuth() {
  const isLoginPage = window.location.pathname.endsWith('login.html');
  const token = API.getToken();

  if (!token && !isLoginPage) {
    window.location.href = '/login.html';
    return false;
  }

  const user = API.getUser();
  const userDisplay = document.getElementById('user-display');
  if (userDisplay && user) {
    userDisplay.textContent = `${user.username} (${user.role})`;
  }

  return true;
}

// Logout handler
async function logout() {
  try {
    await API.request('/api/auth/logout', { method: 'POST' });
  } catch (e) {
    console.warn(e);
  }
  API.clearToken();
  window.location.href = '/login.html';
}

// Dashboard Page
async function initDashboard() {
  if (!checkAuth()) return;

  try {
    const data = await API.request('/api/dashboard');
    document.getElementById('stat-total-suppliers').textContent = data.stats.total_suppliers;
    document.getElementById('stat-verified-suppliers').textContent = data.stats.verified_suppliers;
    document.getElementById('stat-total-jobs').textContent = data.stats.total_jobs;
    document.getElementById('stat-running-jobs').textContent = data.stats.running_jobs;

    // Render Recent Jobs
    const jobsTbody = document.getElementById('recent-jobs-tbody');
    if (jobsTbody) {
      if (data.recent_jobs.length === 0) {
        jobsTbody.innerHTML = `<tr><td colspan="5" style="text-align:center; color: var(--text-muted)">No search jobs yet. Start a new search above.</td></tr>`;
      } else {
        jobsTbody.innerHTML = data.recent_jobs.map(j => `
          <tr>
            <td>#${j.id}</td>
            <td><strong>${escapeHtml(j.original_request)}</strong></td>
            <td><span class="badge ${getStatusBadgeClass(j.status)}">${j.status}</span></td>
            <td>${j.discovered_url_count} URLs / ${j.processed_url_count} Crawled</td>
            <td><a href="/search.html?job_id=${j.id}" class="btn btn-secondary btn-sm">View</a></td>
          </tr>
        `).join('');
      }
    }

    // Render Recent Suppliers
    const suppliersTbody = document.getElementById('recent-suppliers-tbody');
    if (suppliersTbody) {
      if (data.recent_suppliers.length === 0) {
        suppliersTbody.innerHTML = `<tr><td colspan="5" style="text-align:center; color: var(--text-muted)">No suppliers discovered yet.</td></tr>`;
      } else {
        suppliersTbody.innerHTML = data.recent_suppliers.map(s => `
          <tr>
            <td><strong>${escapeHtml(s.company_name)}</strong></td>
            <td>${escapeHtml(s.country || '-')}</td>
            <td>${escapeHtml(s.category || '-')}</td>
            <td><span class="badge ${getStatusBadgeClass(s.verification_status)}">${s.verification_status}</span></td>
            <td><button onclick="viewSupplierDetails(${s.id})" class="btn btn-secondary btn-sm">Inspect</button></td>
          </tr>
        `).join('');
      }
    }
  } catch (err) {
    console.error('Failed to load dashboard:', err);
  }
}

// Search Page
let currentJobId = null;
let pollInterval = null;

async function initSearch() {
  if (!checkAuth()) return;

  const urlParams = new URLSearchParams(window.location.search);
  const jobId = urlParams.get('job_id');
  if (jobId) {
    currentJobId = parseInt(jobId);
    startPollingJob(currentJobId);
  }

  const form = document.getElementById('search-form');
  if (form) {
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const reqInput = document.getElementById('search-input');
      const countryInput = document.getElementById('country-preference');
      const mfgCheck = document.getElementById('prefer-mfg');

      const requestText = reqInput.value.trim();
      if (!requestText) return;

      const countries = countryInput.value
        .split(',')
        .map(c => c.trim())
        .filter(c => c.length > 0);

      try {
        const btn = document.getElementById('start-search-btn');
        btn.disabled = true;
        btn.textContent = 'Queueing Search...';

        const res = await API.request('/api/search', {
          method: 'POST',
          body: JSON.stringify({
            request: requestText,
            preferences: {
              countries,
              exclude_marketplaces: false,
              prioritize_manufacturers: mfgCheck ? mfgCheck.checked : true
            }
          })
        });

        currentJobId = res.job_id;
        history.pushState(null, '', `?job_id=${currentJobId}`);
        startPollingJob(currentJobId);
      } catch (err) {
        alert('Failed to start search: ' + err.message);
      } finally {
        const btn = document.getElementById('start-search-btn');
        btn.disabled = false;
        btn.textContent = 'Start Deep Search';
      }
    });
  }
}

function startPollingJob(jobId) {
  if (pollInterval) clearInterval(pollInterval);
  document.getElementById('job-progress-panel').style.display = 'block';

  pollJobStatus(jobId);
  pollInterval = setInterval(() => pollJobStatus(jobId), 2000);
}

async function pollJobStatus(jobId) {
  try {
    const job = await API.request(`/api/search/${jobId}`);
    document.getElementById('job-id-display').textContent = `#${job.job_id}`;
    document.getElementById('job-status-badge').textContent = job.status;
    document.getElementById('job-status-badge').className = `badge ${getStatusBadgeClass(job.status)}`;
    document.getElementById('job-message').textContent = job.message;

    document.getElementById('metric-rounds').textContent = job.round_count;
    document.getElementById('metric-discovered').textContent = job.discovered_urls;
    document.getElementById('metric-processed').textContent = job.processed_urls;
    document.getElementById('metric-suppliers').textContent = job.suppliers_found;

    const stopBtn = document.getElementById('stop-search-btn');
    if (job.status === 'running' || job.status === 'queued') {
      stopBtn.style.display = 'inline-flex';
      stopBtn.disabled = false;
    } else {
      stopBtn.style.display = 'none';
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
    }

    // Refresh Queries and Results
    await loadJobQueries(jobId);
    await loadJobResults(jobId);
  } catch (err) {
    console.error('Polling error:', err);
  }
}

async function stopCurrentJob() {
  if (!currentJobId) return;
  const stopBtn = document.getElementById('stop-search-btn');
  stopBtn.disabled = true;
  stopBtn.textContent = 'Stopping...';

  try {
    await API.request(`/api/search/${currentJobId}/stop`, { method: 'POST' });
  } catch (err) {
    alert('Stop request failed: ' + err.message);
  }
}

async function loadJobQueries(jobId) {
  try {
    const queries = await API.request(`/api/search/${jobId}/queries`);
    const container = document.getElementById('queries-list');
    if (container && queries.length > 0) {
      container.innerHTML = queries.map(q => `
        <span class="badge badge-info" style="margin-right: 0.5rem; margin-bottom: 0.5rem;">
          ${escapeHtml(q.query_text)} (p.${q.page_number} - ${q.result_count} hits)
        </span>
      `).join('');
    }
  } catch (err) {
    console.warn(err);
  }
}

async function loadJobResults(jobId) {
  try {
    const results = await API.request(`/api/search/${jobId}/results`);
    const tbody = document.getElementById('search-results-tbody');
    const exportBtn = document.getElementById('export-excel-btn');

    if (results.length > 0 && exportBtn) {
      exportBtn.style.display = 'inline-flex';
      exportBtn.onclick = () => downloadExcel(jobId);
    }

    if (tbody) {
      if (results.length === 0) {
        tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--text-muted)">No suppliers extracted yet. Discovery is actively running...</td></tr>`;
      } else {
        tbody.innerHTML = results.map((r, idx) => `
          <tr>
            <td><strong>#${idx + 1}</strong></td>
            <td>
              <span class="badge ${r.relevance_score >= 80 ? 'badge-success' : r.relevance_score >= 50 ? 'badge-warning' : 'badge-info'}">
                ${r.relevance_score.toFixed(1)}%
              </span>
            </td>
            <td>
              <strong>${escapeHtml(r.supplier.company_name)}</strong><br>
              <small style="color: var(--text-muted)">${escapeHtml(r.supplier.domain || '')}</small>
            </td>
            <td>${escapeHtml(r.supplier.country || '-')}</td>
            <td>
              ${r.supplier.email ? `<div>✉️ ${escapeHtml(r.supplier.email)}</div>` : ''}
              ${r.supplier.phone ? `<div>📞 ${escapeHtml(r.supplier.phone)}</div>` : ''}
              ${!r.supplier.email && !r.supplier.phone ? '<span style="color:var(--text-muted)">Website inquiry</span>' : ''}
            </td>
            <td>
              <button onclick="viewSupplierDetails(${r.supplier.id})" class="btn btn-secondary btn-sm">Inspect</button>
            </td>
          </tr>
        `).join('');
      }
    }
  } catch (err) {
    console.warn(err);
  }
}

async function downloadExcel(jobId) {
  const token = API.getToken();
  window.open(`/api/search/${jobId}/export?token=${token}`, '_blank');
}

// Suppliers Directory Page
async function initSuppliers() {
  if (!checkAuth()) return;
  loadSuppliersList();

  const searchInput = document.getElementById('supplier-filter-search');
  if (searchInput) {
    searchInput.addEventListener('input', () => filterSuppliersTable());
  }
  const statusFilter = document.getElementById('supplier-filter-status');
  if (statusFilter) {
    statusFilter.addEventListener('change', () => loadSuppliersList());
  }
}

let cachedSuppliers = [];

async function loadSuppliersList() {
  try {
    const statusSelect = document.getElementById('supplier-filter-status');
    const status = statusSelect ? statusSelect.value : '';
    let endpoint = '/api/suppliers?limit=100';
    if (status) endpoint += `&status=${encodeURIComponent(status)}`;

    const list = await API.request(endpoint);
    cachedSuppliers = list;
    renderSuppliersTable(list);
  } catch (err) {
    console.error('Failed to load suppliers:', err);
  }
}

function filterSuppliersTable() {
  const keyword = document.getElementById('supplier-filter-search').value.toLowerCase();
  const filtered = cachedSuppliers.filter(s =>
    s.company_name.toLowerCase().includes(keyword) ||
    (s.country && s.country.toLowerCase().includes(keyword)) ||
    (s.category && s.category.toLowerCase().includes(keyword)) ||
    (s.domain && s.domain.toLowerCase().includes(keyword))
  );
  renderSuppliersTable(filtered);
}

function renderSuppliersTable(list) {
  const tbody = document.getElementById('suppliers-directory-tbody');
  if (!tbody) return;

  if (list.length === 0) {
    tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--text-muted)">No suppliers match criteria.</td></tr>`;
    return;
  }

  tbody.innerHTML = list.map(s => `
    <tr>
      <td><strong>${escapeHtml(s.company_name)}</strong></td>
      <td>
        ${s.website ? `<a href="${escapeHtml(s.website)}" target="_blank" rel="noopener" style="color:var(--accent)">${escapeHtml(s.domain || 'Visit')}</a>` : '-'}
      </td>
      <td>${escapeHtml(s.country || '-')}</td>
      <td>${escapeHtml(s.category || '-')}</td>
      <td><span class="badge ${getStatusBadgeClass(s.verification_status)}">${s.verification_status}</span></td>
      <td>
        <button onclick="viewSupplierDetails(${s.id})" class="btn btn-secondary btn-sm">Inspect</button>
      </td>
    </tr>
  `).join('');
}

// Supplier Detail Modal
async function viewSupplierDetails(supplierId) {
  try {
    const supplier = await API.request(`/api/suppliers/${supplierId}`);
    const sources = await API.request(`/api/suppliers/${supplierId}/sources`);
    const products = await API.request(`/api/suppliers/${supplierId}/products`);

    document.getElementById('modal-title').textContent = supplier.company_name;
    document.getElementById('modal-domain').textContent = supplier.domain || 'No website';
    document.getElementById('modal-status-badge').textContent = supplier.verification_status;
    document.getElementById('modal-status-badge').className = `badge ${getStatusBadgeClass(supplier.verification_status)}`;

    document.getElementById('modal-country').textContent = `${supplier.city ? supplier.city + ', ' : ''}${supplier.country || 'Global'}`;
    document.getElementById('modal-email').textContent = supplier.email || 'None';
    document.getElementById('modal-phone').textContent = supplier.phone || 'None';
    document.getElementById('modal-desc').textContent = supplier.description || 'No description available';

    // Status Selector
    const statusSelect = document.getElementById('modal-verification-status');
    if (statusSelect) {
      statusSelect.value = supplier.verification_status;
      document.getElementById('modal-save-status-btn').onclick = async () => {
        try {
          await API.request(`/api/suppliers/${supplierId}`, {
            method: 'PATCH',
            body: JSON.stringify({ verification_status: statusSelect.value })
          });
          alert('Supplier status updated successfully');
          closeModal();
          if (window.location.pathname.endsWith('suppliers.html')) {
            loadSuppliersList();
          }
        } catch (e) {
          alert('Failed to update status: ' + e.message);
        }
      };
    }

    // Sources list
    const sourcesContainer = document.getElementById('modal-sources-list');
    if (sourcesContainer) {
      if (sources.length === 0) {
        sourcesContainer.innerHTML = '<p style="color:var(--text-muted)">No source documents recorded.</p>';
      } else {
        sourcesContainer.innerHTML = sources.map(src => `
          <div class="source-item">
            <strong>Source URL:</strong> <a href="${escapeHtml(src.source_url)}" target="_blank" rel="noopener" style="color:var(--accent)">${escapeHtml(src.source_url)}</a>
            <div style="color:var(--text-muted); font-size: 0.8rem; margin-top: 0.25rem;">Extracted: ${src.extracted_at}</div>
          </div>
        `).join('');
      }
    }

    // Products list
    const productsContainer = document.getElementById('modal-products-list');
    if (productsContainer) {
      if (products.length === 0) {
        productsContainer.innerHTML = '<p style="color:var(--text-muted)">No specific items parsed in catalog yet.</p>';
      } else {
        productsContainer.innerHTML = products.map(p => `
          <div class="product-item">
            <strong>${escapeHtml(p.product_name)}</strong>
            ${p.brand ? `<span class="badge badge-info">${escapeHtml(p.brand)}</span>` : ''}
            ${p.model_or_specification ? `<div style="font-size:0.85rem">Spec: ${escapeHtml(p.model_or_specification)}</div>` : ''}
            ${p.price_amount ? `<div style="color:var(--success); font-weight:bold">${p.price_currency || '$'} ${p.price_amount}</div>` : ''}
          </div>
        `).join('');
      }
    }

    document.getElementById('supplier-modal').classList.add('active');
  } catch (err) {
    alert('Failed to load supplier details: ' + err.message);
  }
}

function closeModal() {
  document.getElementById('supplier-modal').classList.remove('active');
}

// Helpers
function getStatusBadgeClass(status) {
  switch (status?.toLowerCase()) {
    case 'running':
    case 'extracted':
    case 'discovered':
      return 'badge-info';
    case 'completed':
    case 'approved':
    case 'verified':
      return 'badge-success';
    case 'stopping':
    case 'contacted':
    case 'queued':
      return 'badge-warning';
    case 'stopped':
    case 'failed':
      return 'badge-danger';
    default:
      return 'badge-info';
  }
}

function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}


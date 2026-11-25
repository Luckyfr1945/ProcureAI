// ProcureAI v2 Client-side Logic (Enterprise UI + Evidence-First)

const API = {
  // Browser authentication uses the HttpOnly cookie; do not persist bearer tokens.
  getToken() { return null; },
  setToken() { localStorage.removeItem('procure_token'); },
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
      credentials: 'same-origin',
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

// ==========================================
// THEME, SIDEBAR & TOAST UTILITIES
// ==========================================
function getResolvedTheme() {
  const stored = localStorage.getItem('procure_theme');
  if (stored === 'dark' || stored === 'light') return stored;
  return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : 'light';
}

function applyTheme(theme) {
  document.documentElement.setAttribute('data-theme', theme);
  document.querySelectorAll('[data-theme-toggle]').forEach((btn) => {
    const isDark = theme === 'dark';
    btn.setAttribute('aria-pressed', String(isDark));
    const labelEl = btn.querySelector('[data-theme-label]');
    const sunEl = btn.querySelector('[data-icon-sun]');
    const moonEl = btn.querySelector('[data-icon-moon]');
    if (labelEl) labelEl.textContent = isDark ? 'Mode Terang' : 'Mode Gelap';
    if (sunEl) sunEl.style.display = isDark ? 'inline-block' : 'none';
    if (moonEl) moonEl.style.display = isDark ? 'none' : 'inline-block';
  });
}

function toggleTheme() {
  const next = getResolvedTheme() === 'dark' ? 'light' : 'dark';
  localStorage.setItem('procure_theme', next);
  applyTheme(next);
}

let shellInitialized = false;

function initShell() {
  applyTheme(getResolvedTheme());
  if (shellInitialized) return;
  shellInitialized = true;

  document.querySelectorAll('[data-theme-toggle]').forEach((btn) => {
    btn.addEventListener('click', toggleTheme);
  });

  document.querySelectorAll('.table-responsive').forEach((region) => {
    region.tabIndex = 0;
    region.setAttribute('role', 'region');
    region.setAttribute('aria-label', 'Tabel data, geser horizontal untuk melihat kolom lainnya');
  });

  const sidebar = document.getElementById('app-sidebar');
  const backdrop = document.getElementById('sidebar-backdrop');
  const menuBtn = document.getElementById('mobile-menu-btn');

  if (menuBtn && sidebar && backdrop) {
    menuBtn.setAttribute('aria-controls', 'app-sidebar');
    menuBtn.setAttribute('aria-expanded', 'false');
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && sidebar.classList.contains('open')) {
        closeSidebar();
        menuBtn.focus();
      }
    });
    menuBtn.addEventListener('click', () => {
      sidebar.classList.toggle('open');
      backdrop.classList.toggle('open');
      menuBtn.setAttribute('aria-expanded', String(sidebar.classList.contains('open')));
    });
    backdrop.addEventListener('click', closeSidebar);
  }

  // Modal/Drawer ESC & Backdrop Close
  const modal = document.getElementById('supplier-modal');
  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) closeModal();
    });
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && modal.classList.contains('active')) {
        closeModal();
      }
    });
  }

  // Drawer tabs
  document.querySelectorAll('.drawer-tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      switchDrawerTab(tab.getAttribute('data-tab'));
    });
  });
}

function closeSidebar() {
  const sidebar = document.getElementById('app-sidebar');
  const backdrop = document.getElementById('sidebar-backdrop');
  if (sidebar) sidebar.classList.remove('open');
  if (backdrop) backdrop.classList.remove('open');
  document.getElementById('mobile-menu-btn')?.setAttribute('aria-expanded', 'false');
}

function showToast(message, type = 'info') {
  let container = document.getElementById('toast-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'toast-container';
    container.className = 'toast-container';
    container.setAttribute('aria-live', 'polite');
    document.body.appendChild(container);
  }

  const icons = {
    success: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
    error: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
    warning: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
    info: '<svg class="toast-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>',
  };

  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  toast.innerHTML = icons[type] || icons.info;

  const msgSpan = document.createElement('div');
  msgSpan.className = 'toast-msg';
  msgSpan.textContent = message;
  toast.appendChild(msgSpan);

  const closeBtn = document.createElement('button');
  closeBtn.className = 'toast-close';
  closeBtn.type = 'button';
  closeBtn.setAttribute('aria-label', 'Tutup notifikasi');
  closeBtn.innerHTML = '&times;';
  closeBtn.onclick = () => toast.remove();
  toast.appendChild(closeBtn);

  container.appendChild(toast);
  setTimeout(() => {
    if (toast.parentNode) toast.remove();
  }, 4200);
}

async function copyText(text, label = 'Teks') {
  if (!text) return;
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const ta = document.createElement('textarea');
      ta.value = text;
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      ta.remove();
    }
    showToast(`${label} disalin ke clipboard`, 'success');
  } catch {
    showToast(`Gagal menyalin ${label.toLowerCase()}`, 'error');
  }
}

// ==========================================
// AUTHENTICATION
// ==========================================
function checkAuth() {
  const isLoginPage = window.location.pathname.endsWith('login.html');
  localStorage.removeItem('procure_token');
  const cachedUser = API.getUser();

  if (!cachedUser && !isLoginPage) {
    window.location.href = '/login.html';
    return false;
  }

  const user = API.getUser();
  const userDisplay = document.getElementById('user-display');
  const userAvatar = document.getElementById('user-avatar');
  const userRole = document.getElementById('user-role');

  if (user) {
    if (userDisplay) userDisplay.textContent = user.username;
    if (userRole) userRole.textContent = user.role || 'purchasing';
    if (userAvatar) userAvatar.textContent = getInitials(user.username || 'U');
  }

  return true;
}

async function logout() {
  try {
    await API.request('/api/v1/auth/logout', { method: 'POST' });
  } catch (e) {
    console.warn(e);
  }
  API.clearToken();
  window.location.href = '/login.html';
}

// ==========================================
// DASHBOARD PAGE
// ==========================================
async function initDashboard() {
  initShell();
  if (!checkAuth()) return;

  try {
    const data = await API.request('/api/v1/dashboard/summary');
    const stats = data.stats || {};
    const totalSup = stats.total_suppliers || 0;
    const verifiedSup = stats.verified_suppliers || 0;
    const runningJobs = stats.running_jobs || 0;

    document.getElementById('stat-total-suppliers').textContent = totalSup;
    document.getElementById('stat-verified-suppliers').textContent = verifiedSup;
    const pubPriceEl = document.getElementById('stat-published-prices');
    if (pubPriceEl) pubPriceEl.textContent = stats.published_prices || 0;
    document.getElementById('stat-total-jobs').textContent = stats.total_jobs || 0;
    document.getElementById('stat-running-jobs').textContent = runningJobs;

    const ratioPct = totalSup > 0 ? Math.round((verifiedSup / totalSup) * 100) : 0;
    const ratioBar = document.getElementById('stat-verified-ratio-bar');
    const ratioText = document.getElementById('stat-verified-ratio-text');
    if (ratioBar) ratioBar.style.width = `${ratioPct}%`;
    if (ratioText) ratioText.textContent = `${ratioPct}% dari total supplier`;

    const runningPill = document.getElementById('stat-running-pill');
    if (runningPill) {
      runningPill.className = runningJobs > 0 ? 'badge badge-info badge-pulse' : 'badge badge-neutral';
    }

    const jobsTbody = document.getElementById('recent-jobs-tbody');
    if (jobsTbody) {
      if (!data.recent_jobs || data.recent_jobs.length === 0) {
        jobsTbody.innerHTML = `
          <tr>
            <td colspan="6">
              <div class="empty-state">
                <div class="empty-title">Belum ada riwayat pencarian</div>
                <div class="empty-desc">Mulai pencarian pertama untuk menemukan supplier beserta kontak, sumber web, dan draft RFQ.</div>
                <a href="/search.html" class="btn btn-sm">Mulai Cari Supplier</a>
              </div>
            </td>
          </tr>
        `;
      } else {
        jobsTbody.innerHTML = data.recent_jobs.map(j => `
          <tr>
            <td><code class="cell-muted" translate="no" title="${escapeHtml(j.id)}">${escapeHtml(j.id.slice(0, 8))}</code></td>
            <td style="max-width:340px;">
              <div class="entity-name">${escapeHtml(j.input_text)}</div>
              <div class="cell-muted">${escapeHtml(formatShortTime(j.created_at))}</div>
            </td>
            <td class="cell-muted">${escapeHtml(formatStageLabel(j.current_stage))}</td>
            <td>${renderStatusBadge(j.status)}</td>
            <td class="num-cell">
              <strong>${j.round_count}</strong> putaran &middot;
              <strong>${j.new_supplier_count}</strong> supplier
            </td>
            <td>
              <a href="/search.html?job_id=${encodeURIComponent(j.id)}" class="btn btn-secondary btn-sm">Buka hasil</a>
            </td>
          </tr>
        `).join('');
      }
    }

    const suppliersTbody = document.getElementById('recent-suppliers-tbody');
    if (suppliersTbody) {
      if (!data.recent_suppliers || data.recent_suppliers.length === 0) {
        suppliersTbody.innerHTML = `
          <tr>
            <td colspan="5">
              <div class="empty-state">
                <div class="empty-title">Belum ada supplier tersimpan</div>
                <div class="empty-desc">Supplier yang ditemukan saat pencarian akan otomatis dirapikan dan tersimpan di sini.</div>
              </div>
            </td>
          </tr>
        `;
      } else {
        suppliersTbody.innerHTML = data.recent_suppliers.map(s => renderSupplierRow(s, false)).join('');
      }
    }
  } catch (err) {
    console.error('Failed to load dashboard:', err);
    showToast('Gagal memuat ringkasan dashboard: ' + err.message, 'error');
  }
}

// ==========================================
// SEARCH PAGE & CONDITIONAL LOCATION UI
// ==========================================
let currentJobId = null;
let currentJobStatus = null;
let currentJobRequirement = null;
let currentJobInputText = '';
let pollInterval = null;
let pollGeneration = 0;

function initLocationControls() {
  const countrySelect = document.getElementById('country-select');
  const countryCustomWrap = document.getElementById('country-custom-wrap');
  const countryCustomInput = document.getElementById('country-custom');
  const indonesiaGroup = document.getElementById('indonesia-region-group');
  const indonesiaSelect = document.getElementById('indonesia-region-select');
  const indonesiaCustomWrap = document.getElementById('indonesia-region-custom-wrap');
  const indonesiaCustomInput = document.getElementById('indonesia-region-custom');
  const langSelect = document.getElementById('lang-preference');

  if (!countrySelect || !indonesiaGroup) return;

  let langManuallyChanged = false;
  if (langSelect) {
    langSelect.addEventListener('change', () => {
      langManuallyChanged = true;
    });
  }

  function syncLocationVisibility() {
    const country = countrySelect.value;
    const isIndonesia = country === 'Indonesia';
    const isCustomCountry = country === 'custom';

    if (countryCustomWrap) {
      countryCustomWrap.style.display = isCustomCountry ? 'block' : 'none';
      if (!isCustomCountry && countryCustomInput) countryCustomInput.value = '';
    }

    if (isIndonesia) {
      indonesiaGroup.style.display = '';
    } else {
      indonesiaGroup.style.display = 'none';
      if (indonesiaSelect) indonesiaSelect.value = '';
      if (indonesiaCustomWrap) indonesiaCustomWrap.style.display = 'none';
      if (indonesiaCustomInput) indonesiaCustomInput.value = '';
    }

    if (langSelect && !langManuallyChanged) {
      langSelect.value = isIndonesia ? 'id,en' : 'en';
    }
  }

  function syncIndonesiaRegionCustom() {
    if (!indonesiaSelect || !indonesiaCustomWrap) return;
    const isCustomRegion = indonesiaSelect.value === 'custom';
    indonesiaCustomWrap.style.display = isCustomRegion ? 'block' : 'none';
    if (!isCustomRegion && indonesiaCustomInput) {
      indonesiaCustomInput.value = '';
    }
  }

  countrySelect.addEventListener('change', syncLocationVisibility);
  if (indonesiaSelect) {
    indonesiaSelect.addEventListener('change', syncIndonesiaRegionCustom);
  }

  syncLocationVisibility();
  syncIndonesiaRegionCustom();
}

function getSelectedTargetRegions() {
  const countrySelect = document.getElementById('country-select');
  const countryCustomInput = document.getElementById('country-custom');
  const indonesiaSelect = document.getElementById('indonesia-region-select');
  const indonesiaCustomInput = document.getElementById('indonesia-region-custom');

  if (!countrySelect) return ['Indonesia'];
  const country = countrySelect.value;

  if (country === 'Indonesia') {
    const regVal = indonesiaSelect ? indonesiaSelect.value : '';
    if (regVal === 'custom') {
      const customReg = indonesiaCustomInput ? indonesiaCustomInput.value.trim() : '';
      return customReg ? [customReg, 'Indonesia'] : ['Indonesia'];
    }
    return regVal ? [regVal, 'Indonesia'] : ['Indonesia'];
  }

  if (country === 'custom') {
    const customCountry = countryCustomInput ? countryCustomInput.value.trim() : '';
    return customCountry ? [customCountry] : ['worldwide'];
  }

  if (country === 'Global') {
    return ['worldwide'];
  }

  return [country];
}

async function initSearch() {
  initShell();
  if (!checkAuth()) return;

  initLocationControls();

  // Bind preset chips
  document.querySelectorAll('.preset-chip').forEach((chip) => {
    chip.addEventListener('click', () => {
      const target = document.getElementById('search-input');
      if (target) {
        target.value = chip.getAttribute('data-prompt') || chip.textContent;
        target.focus();
      }
    });
  });

  // Load recent jobs into quick switcher
  loadRecentJobsSwitcher();

  const urlParams = new URLSearchParams(window.location.search);
  const jobId = urlParams.get('job_id');
  if (jobId) {
    currentJobId = jobId;
    startPollingJob(currentJobId);
  }

  const form = document.getElementById('search-form');
  if (form) {
    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      if (window.confirmPurchasingLeave && !window.confirmPurchasingLeave()) return;
      const reqInput = document.getElementById('search-input');
      const attrInput = document.getElementById('required-attributes');
      const langSelect = document.getElementById('lang-preference');
      const prInput = document.getElementById('pr-number');

      const inputText = reqInput.value.trim();
      if (!inputText) return;

      const targetRegions = getSelectedTargetRegions();
      const requiredAttrs = attrInput && attrInput.value.trim()
        ? attrInput.value.split(',').map(a => a.trim()).filter(Boolean)
        : [];
      const preferredLangs = langSelect && langSelect.value
        ? langSelect.value.split(',').map(l => l.trim()).filter(Boolean)
        : ['id', 'en'];
      const prNumber = prInput && prInput.value.trim() ? prInput.value.trim() : null;

      const btn = document.getElementById('start-search-btn');
      const originalBtnHtml = btn.innerHTML;

      try {
        btn.disabled = true;
        btn.innerHTML = `<svg class="spin" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg> Menyiapkan pencarian…`;

        const res = await API.request('/api/v1/search-jobs', {
          method: 'POST',
          body: JSON.stringify({
            input_text: inputText,
            target_regions: targetRegions,
            preferred_languages: preferredLangs,
            required_attributes: requiredAttrs,
            pr_number: prNumber
          })
        });

        currentJobId = res.job_id;
        history.pushState(null, '', `?job_id=${encodeURIComponent(currentJobId)}`);
        showToast('Pencarian dimulai. Hasil muncul bertahap di bawah.', 'success');
        startPollingJob(currentJobId);
        loadRecentJobsSwitcher();
      } catch (err) {
        showToast('Gagal memulai pencarian: ' + err.message, 'error');
      } finally {
        btn.disabled = false;
        btn.innerHTML = originalBtnHtml;
      }
    });
  }
}

async function loadRecentJobsSwitcher() {
  const select = document.getElementById('recent-jobs-select');
  if (!select) return;
  try {
    const jobs = await API.request('/api/v1/search-jobs');
    if (!Array.isArray(jobs) || jobs.length === 0) {
      select.innerHTML = '<option value="">Belum ada riwayat pencarian</option>';
      return;
    }
    select.innerHTML = '<option value="">Buka pencarian sebelumnya…</option>' + jobs.slice(0, 20).map(j => {
      const label = `${j.input_text.slice(0, 48)}${j.input_text.length > 48 ? '…' : ''} (${formatStatusLabel(j.status)})`;
      const selected = currentJobId && j.id === currentJobId ? 'selected' : '';
      return `<option value="${escapeHtml(j.id)}" ${selected}>${escapeHtml(label)}</option>`;
    }).join('');

    select.onchange = () => {
      if (select.value) {
        if (window.confirmPurchasingLeave && !window.confirmPurchasingLeave()) { select.value = currentJobId || ''; return; }
        currentJobId = select.value;
        history.pushState(null, '', `?job_id=${encodeURIComponent(currentJobId)}`);
        startPollingJob(currentJobId);
      }
    };
  } catch (err) {
    console.warn('Could not load recent jobs list:', err);
  }
}

function startPollingJob(jobId) {
  currentJobStatus = null;
  closeModal();
  if (pollInterval) clearInterval(pollInterval);
  const panel = document.getElementById('job-progress-panel');
  if (panel) panel.style.display = 'block';

  const exportBtn = document.getElementById('export-excel-btn');
  if (exportBtn) exportBtn.style.display = 'none';

  const eventsLog = document.getElementById('job-events-log');
  if (eventsLog) eventsLog.innerHTML = '<div class="log-placeholder">Memuat catatan proses…</div>';
  const queriesList = document.getElementById('queries-list');
  if (queriesList) queriesList.innerHTML = '<div class="log-placeholder">Menyiapkan kata kunci…</div>';
  const evBadge = document.getElementById('events-count-badge');
  if (evBadge) evBadge.textContent = '0';
  const qBadge = document.getElementById('queries-count-badge');
  if (qBadge) qBadge.textContent = '0';

  const copyJobBtn = document.getElementById('copy-job-id-btn');
  if (copyJobBtn) {
    copyJobBtn.onclick = () => copyText(jobId, 'Job ID');
  }

  const generation = ++pollGeneration;
  const tick = async () => {
    if (generation !== pollGeneration || currentJobId !== jobId) return;
    await pollJobStatus(jobId);
    if (generation === pollGeneration && currentJobId === jobId && pollInterval !== null) pollInterval = setTimeout(tick, 2000);
  };
  pollInterval = -1;
  tick();
}

const STAGE_ORDER = ['understand', 'discover', 'triage', 'extract', 'gap_discovery', 'completed'];

function updateStageStepper(currentStage, jobStatus) {
  const stepper = document.getElementById('pipeline-stepper');
  if (!stepper) return;
  const steps = stepper.querySelectorAll('.stage-step');
  const activeIdx = STAGE_ORDER.indexOf(currentStage || 'understand');
  const isErrorState = jobStatus === 'failed' || jobStatus === 'stopped';

  steps.forEach((step, idx) => {
    const stageName = step.getAttribute('data-stage');
    const stepIdx = STAGE_ORDER.indexOf(stageName);
    step.classList.remove('active', 'done', 'failed');
    const numEl = step.querySelector('.stage-num');

    if (jobStatus === 'completed' || stepIdx < activeIdx) {
      step.classList.add('done');
      if (numEl) numEl.innerHTML = '&#10003;';
    } else if (stepIdx === activeIdx) {
      step.classList.add(isErrorState ? 'failed' : 'active');
      if (numEl) numEl.textContent = String(idx + 1);
    } else {
      if (numEl) numEl.textContent = String(idx + 1);
    }
  });
}

async function pollJobStatus(jobId) {
  try {
    const job = await API.request(`/api/v1/search-jobs/${encodeURIComponent(jobId)}`);
    if (currentJobId !== jobId) return;
    currentJobStatus = job.status;
    document.getElementById('job-id-display').textContent = job.job_id;

    const badgeEl = document.getElementById('job-status-badge');
    if (badgeEl) {
      const isLive = job.status === 'running' || job.status === 'queued';
      badgeEl.className = `badge ${getStatusBadgeClass(job.status)}${isLive ? ' badge-pulse' : ''}`;
      badgeEl.innerHTML = `<span class="badge-dot"></span><span>${escapeHtml(formatStatusLabel(job.status))}</span>`;
    }

    const jobPromptEl = document.getElementById('job-prompt-preview');
    if (jobPromptEl && job.input_text) {
      jobPromptEl.textContent = job.input_text;
    }
    currentJobInputText = job.input_text || '';
    currentJobRequirement = job.requirement || null;

    const errBanner = document.getElementById('job-error-banner');
    const errText = document.getElementById('job-error-text');
    if (errBanner && errText) {
      if (job.error_message) {
        errBanner.style.display = 'flex';
        errText.textContent = job.error_message;
      } else {
        errBanner.style.display = 'none';
      }
    }

    updateStageStepper(job.current_stage, job.status);

    document.getElementById('metric-rounds').textContent = job.round_count;
    document.getElementById('metric-queries').textContent = job.query_count;
    document.getElementById('metric-suppliers').textContent = job.new_supplier_count;

    // Show Analyzed Requirement Schema if ready
    const reqBox = document.getElementById('req-schema-box');
    const reqContent = document.getElementById('req-schema-content');
    if (reqBox && reqContent && job.requirement) {
      reqBox.style.display = 'block';
      const r = job.requirement;
      const attrs = (r.required_attributes || [])
        .map(a => `<span class="attr-tag">${escapeHtml(a)}</span>`)
        .join('');
      const regionsText = (r.target_regions && r.target_regions.length > 0)
        ? r.target_regions.join(', ')
        : 'Global';
      reqContent.innerHTML = `
        <div class="req-meta-row">
          <div><span>Barang / Jasa:</span> <strong>${escapeHtml(r.product_name)}</strong></div>
          <div><span>Kategori:</span> <strong>${escapeHtml(formatCategory(r.category))}</strong></div>
          <div><span>Lokasi Target:</span> <strong>${escapeHtml(regionsText)}</strong></div>
          ${r.pr_number ? `<div><span>No. PR:</span> <code class="attr-tag" translate="no">${escapeHtml(r.pr_number)}</code></div>` : ''}
          ${r.brand ? `<div><span>Merek:</span> <strong translate="no">${escapeHtml(r.brand)}</strong></div>` : ''}
          ${r.part_number ? `<div><span>Part number:</span> <code class="attr-tag" translate="no">${escapeHtml(r.part_number)}</code></div>` : ''}
        </div>
        ${attrs ? `<div class="req-attrs-list"><span class="cell-muted">Syarat wajib:</span>${attrs}</div>` : ''}
      `;
    } else if (reqBox) {
      reqBox.style.display = 'none';
    }

    const stopBtn = document.getElementById('stop-search-btn');
    if (job.status === 'running' || job.status === 'queued' || job.status === 'stopping') {
      stopBtn.style.display = 'inline-flex';
      stopBtn.disabled = job.status === 'stopping';
      stopBtn.textContent = job.status === 'stopping' ? 'Menghentikan…' : 'Hentikan pencarian';
    } else {
      stopBtn.style.display = 'none';
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
    }

    await Promise.all([
      loadJobEvents(jobId),
      loadJobQueries(jobId),
      loadJobResults(jobId),
    ]);
  } catch (err) {
    console.error('Polling error:', err);
  }
}

async function stopCurrentJob() {
  if (!currentJobId) return;
  const stopBtn = document.getElementById('stop-search-btn');
  stopBtn.disabled = true;
  stopBtn.textContent = 'Menghentikan…';

  try {
    await API.request(`/api/v1/search-jobs/${encodeURIComponent(currentJobId)}/stop`, { method: 'POST' });
    showToast('Sinyal penghentian dikirim ke worker', 'warning');
  } catch (err) {
    showToast('Gagal menghentikan job: ' + err.message, 'error');
  }
}

async function loadJobEvents(jobId) {
  try {
    const events = await API.request(`/api/v1/search-jobs/${encodeURIComponent(jobId)}/events`);
    if (currentJobId !== jobId) return;
    const container = document.getElementById('job-events-log');
    const countEl = document.getElementById('events-count-badge');
    if (countEl) countEl.textContent = events.length;

    if (container) {
      if (events.length === 0) {
        container.innerHTML = '<div class="log-placeholder">Menunggu aktivitas…</div>';
      } else {
        container.innerHTML = events.map(ev => `
          <div class="event-line">
            <span class="badge badge-neutral">${escapeHtml(formatStageLabel(ev.stage))}</span>
            <span class="event-msg">${escapeHtml(ev.message || '-')}</span>
            <span class="event-time">${escapeHtml(formatShortTime(ev.created_at))}</span>
          </div>
        `).join('');
        container.scrollTop = container.scrollHeight;
      }
    }
  } catch (err) {
    console.warn(err);
  }
}

async function loadJobQueries(jobId) {
  try {
    const queries = await API.request(`/api/v1/search-jobs/${encodeURIComponent(jobId)}/queries`);
    if (currentJobId !== jobId) return;
    const container = document.getElementById('queries-list');
    const countEl = document.getElementById('queries-count-badge');
    if (countEl) countEl.textContent = queries.length;

    if (container) {
      if (queries.length === 0) {
        container.innerHTML = '<div class="log-placeholder">Menyiapkan kata kunci…</div>';
      } else {
        container.innerHTML = queries.map(q => `
          <div class="query-row">
            <div class="query-main">
              <span class="badge badge-neutral">${escapeHtml(q.query_type || 'general')}</span>
              <span class="query-text" title="${escapeHtml(q.query_text)}">${escapeHtml(q.query_text)}</span>
            </div>
            <span class="query-hits">Hal. ${q.page_or_offset} &middot; ${q.result_count} hasil</span>
          </div>
        `).join('');
      }
    }
  } catch (err) {
    console.warn(err);
  }
}

async function loadJobResults(jobId) {
  try {
    const results = await API.request(`/api/v1/search-jobs/${encodeURIComponent(jobId)}/results`);
    if (currentJobId !== jobId) return;
    if (window.setPurchasingCandidates) window.setPurchasingCandidates(jobId, results);
    const tbody = document.getElementById('search-results-tbody');
    const exportBtn = document.getElementById('export-excel-btn');
    const resultsCount = document.getElementById('results-count-badge');

    if (resultsCount) resultsCount.textContent = `${results.length} kandidat`;
    const actualCount = document.getElementById('metric-suppliers');
    if (actualCount) actualCount.textContent = results.length;

    if (results.length > 0 && exportBtn) {
      exportBtn.style.display = 'inline-flex';
      exportBtn.onclick = () => downloadExcel(jobId);
    } else if (exportBtn) {
      exportBtn.style.display = 'none';
    }

    if (tbody) {
      if (results.length === 0) {
        tbody.innerHTML = `
          <tr>
            <td colspan="6">
              <div class="empty-state" style="padding:2rem 1rem;">
                <div class="empty-title">${['queued', 'running', 'stopping'].includes(currentJobStatus) ? 'Menelusuri kandidat supplier' : 'Belum ada kandidat dengan bukti produk dan lokasi yang sesuai'}</div>
                <div class="empty-desc">${['queued', 'running', 'stopping'].includes(currentJobStatus) ? 'Hasil yang memiliki bukti produk dan lokasi sesuai akan muncul di sini.' : 'Coba istilah barang lain atau periksa spesifikasi dan lokasi. Penawaran lama yang belum diperiksa tidak dihitung sebagai kecocokan.'}</div>
              </div>
            </td>
          </tr>
        `;
      } else {
        tbody.innerHTML = results.map((r) => {
          const s = r.supplier;
          const offers = r.offers || [];
          const firstOffer = offers.length > 0 ? offers[0].offer : null;
          const safeSite = safeUrl(s.website);
          const fitPct = Math.max(0, Math.min(100, Math.round(r.product_fit_score || 0)));
          const evPct = Math.max(0, Math.min(100, Math.round(r.evidence_completeness_score || 0)));

          let priceHtml = `<span class="cell-muted">Belum ada harga</span>`;
          if (firstOffer && firstOffer.price_amount && firstOffer.price_amount > 0) {
            priceHtml = `<div class="num-cell"><strong>${escapeHtml(formatPrice(firstOffer.price_amount, firstOffer.currency))}</strong></div>`;
          }

          return `
            <tr>
              <td class="num-cell"><strong>${r.rank_position}</strong></td>
              <td>
                <div class="dual-score">
                  <div class="score-meter-row">
                    <div class="score-meter-head">
                      <span class="score-meter-label">Kecocokan</span>
                      <span class="score-meter-val">${fitPct}%</span>
                    </div>
                    <div class="score-meter-track">
                      <div class="score-meter-fill" style="width:${fitPct}%"></div>
                    </div>
                  </div>
                  <div class="score-meter-row">
                    <div class="score-meter-head">
                      <span class="score-meter-label">Bukti</span>
                      <span class="score-meter-val">${evPct}%</span>
                    </div>
                    <div class="score-meter-track ${evPct >= 70 ? 'evidence' : 'warning'}">
                      <div class="score-meter-fill ${evPct >= 70 ? 'evidence' : 'warning'}" style="width:${evPct}%"></div>
                    </div>
                  </div>
                </div>
              </td>
              <td>
                <div class="entity-cell">
                  <div class="entity-name">${escapeHtml(s.canonical_name)}</div>
                  <div class="entity-meta">
                    <span class="cell-muted">${escapeHtml(formatSupplierType(s.supplier_type))}</span>
                    ${safeSite ? `<span class="cell-muted" aria-hidden="true">·</span><a href="${escapeHtml(safeSite)}" target="_blank" rel="noopener noreferrer" class="entity-link" translate="no">${escapeHtml(s.normalized_domain || safeSite)}${EXTERNAL_ICON}</a>` : ''}
                  </div>
                </div>
              </td>
              <td>${escapeHtml(s.country_name || s.country_code || '–')}</td>
              <td>
                ${priceHtml}
                ${s.public_email ? `<div class="cell-muted" translate="no">${escapeHtml(s.public_email)}</div>` : ''}
                ${s.public_phone ? `<div class="cell-muted num-cell">${escapeHtml(s.public_phone)}</div>` : ''}
              </td>
              <td>
                <div class="cell-actions">
                  ${renderStatusBadge(s.status)}
                  <button onclick="viewSupplierDetails(${s.id})" class="btn btn-secondary btn-sm">
                    Detail &amp; RFQ (${(r.evidence_items || []).length})
                  </button>
                  <button type="button" class="btn btn-secondary btn-sm" onclick="editPurchasingReview(${s.id})">Shortlist / Catat penawaran</button>
                </div>
              </td>
            </tr>
          `;
        }).join('');
      }
    }
  } catch (err) {
    console.warn(err);
  }
}

async function downloadExcel(jobId) {
  const btn = document.getElementById('export-excel-btn');
  const orig = btn ? btn.innerHTML : '';
  try {
    if (btn) {
      btn.disabled = true;
      btn.textContent = 'Menyiapkan Excel…';
    }
    const token = API.getToken();
    const res = await fetch(`/api/v1/search-jobs/${encodeURIComponent(jobId)}/export.xlsx`, {
      headers: token ? { 'Authorization': `Bearer ${token}` } : {}
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `procureai_job_${jobId}_results.xlsx`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showToast('File Excel berhasil diunduh', 'success');
  } catch (err) {
    showToast('Gagal mengunduh file Excel: ' + err.message, 'error');
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = orig;
    }
  }
}

// ==========================================
// SUPPLIERS DIRECTORY PAGE
// ==========================================
async function initSuppliers() {
  initShell();
  if (!checkAuth()) return;
  loadSuppliersList();

  document.getElementById('reset-supplier-filters')?.addEventListener('click', () => {
    document.getElementById('supplier-filter-search').value = '';
    document.getElementById('supplier-filter-status').value = '';
    directoryOffset = 0;
    loadSuppliersList();
  });
  const searchInput = document.getElementById('supplier-filter-search');
  if (searchInput) {
    let debounce;
    searchInput.addEventListener('input', () => { clearTimeout(debounce); directoryOffset = 0; debounce = setTimeout(loadSuppliersList, 250); });
  }
  const statusFilter = document.getElementById('supplier-filter-status');
  if (statusFilter) {
    statusFilter.addEventListener('change', () => { directoryOffset = 0; loadSuppliersList(); });
  }
}

let cachedSuppliers = [];
let directoryOffset = 0;
let directoryGeneration = 0;

async function loadSuppliersList() {
  const generation = ++directoryGeneration;
  try {
    const statusSelect = document.getElementById('supplier-filter-status');
    const status = statusSelect ? statusSelect.value : '';
    const keyword = document.getElementById('supplier-filter-search')?.value.trim() || '';
    let endpoint = `/api/v1/suppliers?limit=51&offset=${directoryOffset}&q=${encodeURIComponent(keyword)}`;
    if (status) endpoint += `&status=${encodeURIComponent(status)}`;

    const list = await API.request(endpoint);
    if (generation !== directoryGeneration) return;
    cachedSuppliers = list.slice(0, 50);
    let pager = document.getElementById('directory-pagination');
    if (!pager) {
      pager = document.createElement('div'); pager.id = 'directory-pagination'; pager.className = 'page-header-actions';
      document.getElementById('suppliers-directory-tbody').closest('.card').append(pager);
    }
    pager.replaceChildren();
    for (const [label, disabled, delta] of [['Sebelumnya', directoryOffset === 0, -50], ['Berikutnya', list.length <= 50, 50]]) {
      const button = document.createElement('button'); button.type = 'button'; button.className = 'btn btn-secondary btn-sm'; button.textContent = label; button.disabled = disabled;
      button.addEventListener('click', () => { directoryOffset += delta; loadSuppliersList(); }); pager.append(button);
    }
    filterSuppliersTable();
  } catch (err) {
    console.error('Failed to load suppliers:', err);
    showToast('Gagal memuat daftar supplier: ' + err.message, 'error');
  }
}

function filterSuppliersTable() {
  const countEl = document.getElementById('suppliers-count-badge');
  if (countEl) countEl.textContent = cachedSuppliers.length ? `Menampilkan ${directoryOffset + 1}–${directoryOffset + cachedSuppliers.length}` : 'Tidak ada hasil';
  renderSuppliersTable(cachedSuppliers);
}

function renderSuppliersTable(list) {
  const tbody = document.getElementById('suppliers-directory-tbody');
  if (!tbody) return;

  if (list.length === 0) {
    tbody.innerHTML = `
      <tr>
        <td colspan="6">
          <div class="empty-state">
            <div class="empty-title">Tidak ada supplier ditemukan</div>
            <div class="empty-desc">Coba ubah kata kunci pencarian atau filter status verifikasi Anda.</div>
          </div>
        </td>
      </tr>
    `;
    return;
  }

  tbody.innerHTML = list.map(s => renderSupplierRow(s, true)).join('');
}

function renderSupplierRow(s, showSeparateWebsiteCol) {
  const safeSite = safeUrl(s.website);
  const websiteLink = safeSite
    ? `<a href="${escapeHtml(safeSite)}" target="_blank" rel="noopener noreferrer" class="entity-link" translate="no">${escapeHtml(s.normalized_domain || safeSite)}${EXTERNAL_ICON}</a>`
    : `<span class="cell-muted">–</span>`;

  if (showSeparateWebsiteCol) {
    return `
      <tr>
        <td>
          <div class="entity-cell">
            <div class="entity-name">${escapeHtml(s.canonical_name)}</div>
            ${s.public_email ? `<div class="cell-muted" translate="no">${escapeHtml(s.public_email)}</div>` : ''}
          </div>
        </td>
        <td>${websiteLink}</td>
        <td>${escapeHtml(s.country_name || s.country_code || '–')}</td>
        <td>
          <div>${escapeHtml(formatSupplierType(s.supplier_type))}</div>
          <div class="cell-muted">${escapeHtml(formatCategory(s.business_category))}</div>
        </td>
        <td>${renderStatusBadge(s.status)}</td>
        <td>
          <button onclick="viewSupplierDetails(${s.id})" class="btn btn-secondary btn-sm">Detail &amp; RFQ</button>
        </td>
      </tr>
    `;
  }

  return `
    <tr>
      <td>
        <div class="entity-cell">
          <div class="entity-name">${escapeHtml(s.canonical_name)}</div>
          ${safeSite ? websiteLink : ''}
        </div>
      </td>
      <td>${escapeHtml(s.country_name || s.country_code || '–')}</td>
      <td>
        <div>${escapeHtml(formatSupplierType(s.supplier_type))}</div>
        <div class="cell-muted">${escapeHtml(formatCategory(s.business_category))}</div>
      </td>
      <td>${renderStatusBadge(s.status)}</td>
      <td>
        <button onclick="viewSupplierDetails(${s.id})" class="btn btn-secondary btn-sm">Detail &amp; RFQ</button>
      </td>
    </tr>
  `;
}

// ==========================================
// EVIDENCE PROVENANCE, VERIFICATION & DRAFT RFQ
// ==========================================
let currentDrawerSupplier = null;
let drawerRequestGeneration = 0;
let currentDrawerOffers = [];
let rfqChannel = 'email';
let rfqLang = 'id';
let rfqProductName = '';

function switchDrawerTab(tabName) {
  document.querySelectorAll('.drawer-tab').forEach((tab) => {
    const active = tab.getAttribute('data-tab') === tabName;
    tab.classList.toggle('active', active);
    tab.setAttribute('aria-selected', String(active));
  });
  document.querySelectorAll('.drawer-panel').forEach((panel) => {
    panel.classList.toggle('active', panel.getAttribute('data-panel') === tabName);
  });
}

function normalizeWhatsAppDigits(rawPhone) {
  if (!rawPhone) return '';
  let digits = String(rawPhone).replace(/[^\d]/g, '');
  if (!digits || digits.length < 8) return '';
  if (digits.startsWith('0') && digits.length >= 10) {
    digits = '62' + digits.slice(1);
  }
  return digits;
}

function buildRfqDraft({ supplier, offers, requirement, jobInputText, channel, lang }) {
  const vendorName = supplier?.canonical_name || 'Tim Sales / Bapak / Ibu';
  const productName =
    requirement?.product_name ||
    (offers && offers.length > 0 && offers[0].description) ||
    jobInputText ||
    '[Nama Barang / Jasa]';
  const brand = requirement?.brand || '';
  const partNumber = requirement?.part_number || '';
  const prNumber = requirement?.pr_number || '';
  const reqAttrs = (requirement?.required_attributes || []).filter(Boolean);
  const specLine = [
    brand ? `Merek / Brand: ${brand}` : '',
    partNumber ? `Tipe / Part Number: ${partNumber}` : '',
    reqAttrs.length > 0 ? `Spesifikasi / Syarat: ${reqAttrs.join(', ')}` : ''
  ].filter(Boolean);

  if (lang === 'en') {
    const subject = prNumber
      ? `Request for Quotation (RFQ - ${prNumber}): ${productName}`
      : `Request for Quotation (RFQ): ${productName}`;

    const enLines = [
      channel === 'email' ? `Dear Sales Team of ${vendorName},` : `Hello ${vendorName} Sales Team,`,
      ``,
      `We are from the Purchasing Department and would like to request an official quotation (RFQ) for the following item:`,
      `- Item / Description: ${productName}`,
      ...(brand ? [`- Brand: ${brand}`] : []),
      ...(partNumber ? [`- Model / Part Number: ${partNumber}`] : []),
      ...(reqAttrs.length > 0 ? [`- Key Requirements: ${reqAttrs.join(', ')}`] : []),
      `- Quantity: [Fill Qty & Unit]`,
      ...(prNumber ? [`- Reference PR No.: ${prNumber}`] : []),
      ``,
      `Could you please provide your quotation including:`,
      `1. Unit price (currency & tax/VAT status) and MOQ`,
      `2. Stock availability & estimated delivery lead time`,
      `3. Payment terms & quotation validity period`,
      `4. Product datasheet / authenticity warranty (if applicable)`,
      ``,
      `Thank you for your prompt response.`,
      `Best regards,`,
      `Purchasing Team`
    ];
    return { subject, body: enLines.join('\n') };
  }

  const subject = prNumber
    ? `Permintaan Penawaran Harga (RFQ - ${prNumber}): ${productName}`
    : `Permintaan Penawaran Harga (RFQ): ${productName}`;

  const idLines = [
    channel === 'email'
      ? `Yth. Tim Sales ${vendorName},`
      : `Halo Tim Sales ${vendorName},`,
    ``,
    `Perkenalkan, kami dari bagian Purchasing ingin mengajukan permintaan penawaran harga resmi (RFQ) untuk kebutuhan pengadaan berikut:`,
    `- Nama Barang / Jasa: ${productName}`,
    ...(specLine.length > 0 ? specLine.map(line => `- ${line}`) : []),
    `- Jumlah Kebutuhan: [Isi Qty & Satuan]`,
    ...(prNumber ? [`- Nomor Referensi PR: ${prNumber}`] : []),
    ``,
    `Mohon kesediaannya untuk mengirimkan informasi penawaran beserta rincian:`,
    `1. Harga satuan (beserta status PPN) dan minimal pembelian (MOQ)`,
    `2. Ketersediaan stok / estimasi waktu pengiriman (lead time)`,
    `3. Syarat pembayaran (TOP) dan masa berlaku penawaran`,
    `4. Brosur teknis / surat garansi keaslian (jika tersedia)`,
    ``,
    `Terima kasih atas perhatian dan kerja samanya.`,
    `Hormat kami,`,
    `Tim Purchasing`
  ];
  return { subject, body: idLines.join('\n') };
}

function renderRfqPanel() {
  const container = document.getElementById('modal-rfq-panel');
  if (!container) return;

  const supplier = currentDrawerSupplier;
  const hasJobContext = window.location.pathname.endsWith('search.html') && currentJobRequirement;
  const offers = hasJobContext ? currentDrawerOffers : [];
  const { subject, body } = buildRfqDraft({
    supplier,
    offers,
    requirement: hasJobContext ? currentJobRequirement : (rfqProductName ? { product_name: rfqProductName } : null),
    jobInputText: hasJobContext ? currentJobInputText : '',
    channel: rfqChannel,
    lang: rfqLang,
  });

  const email = supplier?.public_email || '';
  const waDigits = normalizeWhatsAppDigits(supplier?.public_phone || '');

  container.innerHTML = `
    <div class="rfq-box">
      ${!hasJobContext ? `<div class="form-group"><label for="rfq-item-name">Barang yang akan diminta penawarannya</label><input id="rfq-item-name" list="rfq-product-options" value="${escapeHtml(rfqProductName)}" placeholder="Isi nama barang dan spesifikasi"><datalist id="rfq-product-options">${currentDrawerOffers.map(o => `<option value="${escapeHtml(o.description || '')}"></option>`).join('')}</datalist><p class="field-help">Tentukan barang agar draft tidak memakai produk supplier lain secara otomatis.</p></div>` : ''}
      <div class="rfq-controls">
        <div class="rfq-pill-group" role="group" aria-label="Pilih saluran pengiriman RFQ">
          <button type="button" class="rfq-pill-btn ${rfqChannel === 'email' ? 'active' : ''}" data-rfq-channel="email">Email</button>
          <button type="button" class="rfq-pill-btn ${rfqChannel === 'whatsapp' ? 'active' : ''}" data-rfq-channel="whatsapp">WhatsApp</button>
        </div>
        <div class="rfq-pill-group" role="group" aria-label="Pilih bahasa pesan RFQ">
          <button type="button" class="rfq-pill-btn ${rfqLang === 'id' ? 'active' : ''}" data-rfq-lang="id">Bahasa Indonesia</button>
          <button type="button" class="rfq-pill-btn ${rfqLang === 'en' ? 'active' : ''}" data-rfq-lang="en">English</button>
        </div>
      </div>

      ${rfqChannel === 'email' ? `
        <div class="form-group">
          <label for="rfq-subject-input">Subjek email</label>
          <input type="text" id="rfq-subject-input" autocomplete="off" value="${escapeHtml(subject)}">
        </div>
      ` : ''}

      <div class="form-group" style="margin-bottom: 0;">
        <label for="rfq-draft-textarea">Isi pesan <span class="label-hint">(bisa diedit)</span></label>
        <textarea id="rfq-draft-textarea" class="rfq-textarea" rows="12" autocomplete="off"></textarea>
      </div>

      <div class="rfq-actions">
        <button type="button" class="btn btn-sm" id="rfq-copy-btn">Salin draft</button>
        ${email
          ? `<button type="button" class="btn btn-secondary btn-sm" id="rfq-open-email-btn">Buka di aplikasi email (<span translate="no">${escapeHtml(email)}</span>)</button>`
          : `<span class="cell-muted">Email supplier tidak tercantum di web.</span>`}
        ${waDigits
          ? `<button type="button" class="btn btn-success btn-sm" id="rfq-open-wa-btn">Buka WhatsApp (${escapeHtml(supplier.public_phone)})</button>`
          : `<span class="cell-muted">Nomor telepon/WA supplier tidak tercantum.</span>`}
      </div>
    </div>
  `;

  const itemInput = document.getElementById('rfq-item-name');
  if (itemInput) itemInput.addEventListener('change', () => { rfqProductName = itemInput.value.trim(); renderRfqPanel(); });
  if (!hasJobContext && !rfqProductName) container.querySelectorAll('#rfq-copy-btn, #rfq-open-email-btn, #rfq-open-wa-btn').forEach(button => { button.disabled = true; });
  const textarea = document.getElementById('rfq-draft-textarea');
  if (textarea) textarea.value = body;

  container.querySelectorAll('[data-rfq-channel]').forEach((btn) => {
    btn.addEventListener('click', () => {
      rfqChannel = btn.getAttribute('data-rfq-channel');
      renderRfqPanel();
    });
  });

  container.querySelectorAll('[data-rfq-lang]').forEach((btn) => {
    btn.addEventListener('click', () => {
      rfqLang = btn.getAttribute('data-rfq-lang');
      renderRfqPanel();
    });
  });

  const copyBtn = document.getElementById('rfq-copy-btn');
  if (copyBtn && textarea) {
    copyBtn.addEventListener('click', () => {
      const subjInput = document.getElementById('rfq-subject-input');
      const fullText = (rfqChannel === 'email' && subjInput && subjInput.value.trim())
        ? `Subjek: ${subjInput.value.trim()}\n\n${textarea.value}`
        : textarea.value;
      copyText(fullText, 'Draft RFQ');
    });
  }

  const emailBtn = document.getElementById('rfq-open-email-btn');
  if (emailBtn && email && textarea) {
    emailBtn.addEventListener('click', () => {
      const subjInput = document.getElementById('rfq-subject-input');
      const finalSubj = subjInput ? subjInput.value : subject;
      const mailto = `mailto:${encodeURIComponent(email)}?subject=${encodeURIComponent(finalSubj)}&body=${encodeURIComponent(textarea.value)}`;
      window.location.href = mailto;
    });
  }

  const waBtn = document.getElementById('rfq-open-wa-btn');
  if (waBtn && waDigits && textarea) {
    waBtn.addEventListener('click', () => {
      const waUrl = `https://wa.me/${encodeURIComponent(waDigits)}?text=${encodeURIComponent(textarea.value)}`;
      window.open(waUrl, '_blank', 'noopener,noreferrer');
    });
  }
}

async function viewSupplierDetails(supplierId) {
  const generation = ++drawerRequestGeneration;
  try {
    const drawerJob = window.location.pathname.endsWith('search.html') ? currentJobId : null;
    const data = await API.request(`/api/v1/suppliers/${supplierId}${drawerJob ? `?job_id=${encodeURIComponent(drawerJob)}` : ''}`);
    if (generation !== drawerRequestGeneration || (drawerJob && drawerJob !== currentJobId)) return;
    const s = data.supplier;
    const sources = data.sources || [];
    const evidenceItems = data.evidence_items || [];
    const offers = data.offers || [];

    currentDrawerSupplier = s;
    currentDrawerOffers = offers;
    rfqProductName = '';
    const countryStr = `${s.country_name || ''} ${s.country_code || ''}`.toLowerCase();
    const isForeign = countryStr.trim() !== '' && !countryStr.includes('indonesia') && s.country_code !== 'ID';
    rfqLang = isForeign ? 'en' : 'id';

    document.getElementById('modal-title').textContent = s.canonical_name;
    const safeSite = safeUrl(s.website);
    document.getElementById('modal-domain').innerHTML = safeSite
      ? `<a href="${escapeHtml(safeSite)}" target="_blank" rel="noopener noreferrer" class="entity-link" translate="no">${escapeHtml(s.website)}${EXTERNAL_ICON}</a>`
      : `<span class="cell-muted">${escapeHtml(s.normalized_domain || 'Website tidak tersedia')}</span>`;

    const badgeEl = document.getElementById('modal-status-badge');
    badgeEl.className = `badge ${getStatusBadgeClass(s.status)}`;
    badgeEl.innerHTML = `<span class="badge-dot"></span><span>${escapeHtml(formatStatusLabel(s.status))}</span>`;

    document.getElementById('modal-country').textContent = s.country_name || s.country_code || 'Tidak dipublikasikan';
    document.getElementById('modal-type').textContent = formatSupplierType(s.supplier_type);

    for (const [id, value, label] of [['modal-email', s.public_email, 'Email'], ['modal-phone', s.public_phone, 'Telepon']]) {
      const container = document.getElementById(id);
      renderContactValue(container, value, label);
    }

    document.getElementById('modal-address').textContent = s.address || 'Tidak dipublikasikan';

    // Tab counts
    const offersCount = document.getElementById('tab-count-offers');
    const evidenceCount = document.getElementById('tab-count-evidence');
    const sourcesCount = document.getElementById('tab-count-sources');
    if (offersCount) offersCount.textContent = offers.length;
    if (evidenceCount) evidenceCount.textContent = evidenceItems.length;
    if (sourcesCount) sourcesCount.textContent = sources.length;

    // One-click Purchasing Verification
    const verifyBtn = document.getElementById('modal-verify-btn');
    if (verifyBtn) {
      verifyBtn.disabled = false;
      verifyBtn.onclick = async () => {
        const orig = verifyBtn.textContent;
        try {
          verifyBtn.disabled = true;
          verifyBtn.textContent = 'Menyimpan…';
          await API.request(`/api/v1/suppliers/${supplierId}/verify`, {
            method: 'POST',
            body: JSON.stringify({ verification_status: 'verified_by_purchasing' })
          });
          showToast('Supplier berhasil diverifikasi oleh tim Purchasing', 'success');
          await viewSupplierDetails(supplierId);
          refreshCurrentView();
        } catch (e) {
          showToast('Gagal verifikasi supplier: ' + e.message, 'error');
        } finally {
          verifyBtn.disabled = false;
          verifyBtn.textContent = orig;
        }
      };
    }

    // Status Dropdown Update
    const statusSelect = document.getElementById('modal-verification-status');
    const saveStatusBtn = document.getElementById('modal-save-status-btn');
    if (statusSelect && saveStatusBtn) {
      statusSelect.value = s.status;
      saveStatusBtn.disabled = false;
      saveStatusBtn.onclick = async () => {
        const orig = saveStatusBtn.textContent;
        try {
          saveStatusBtn.disabled = true;
          saveStatusBtn.textContent = 'Menyimpan…';
          await API.request(`/api/v1/suppliers/${supplierId}`, {
            method: 'PATCH',
            body: JSON.stringify({ status: statusSelect.value })
          });
          showToast(`Status supplier diubah menjadi "${formatStatusLabel(statusSelect.value)}"`, 'success');
          await viewSupplierDetails(supplierId);
          refreshCurrentView();
        } catch (e) {
          showToast('Gagal memperbarui status: ' + e.message, 'error');
        } finally {
          saveStatusBtn.disabled = false;
          saveStatusBtn.textContent = orig;
        }
      };
    }

    // Offers & Dynamic Attributes Table
    const offersContainer = document.getElementById('modal-offers-list');
    if (offersContainer) {
      if (offers.length === 0) {
        offersContainer.innerHTML = `<div class="empty-state"><div class="empty-title">Belum ada data produk</div><div class="empty-desc">Halaman supplier ini belum memuat produk yang bisa dibaca. Buka tab Sumber untuk mengecek langsung.</div></div>`;
      } else {
        offersContainer.innerHTML = offers.map(off => {
          let attrsHtml = '';
          try {
            const parsed = JSON.parse(off.attributes_json || '{}');
            const keys = Object.keys(parsed).filter(k => parsed[k] !== null && parsed[k] !== '');
            if (keys.length > 0) {
              attrsHtml = `<div class="attr-grid">` + keys.map(k => `
                <div class="attr-cell"><strong>${escapeHtml(k)}:</strong> ${escapeHtml(String(parsed[k]))}</div>
              `).join('') + `</div>`;
            }
          } catch {}

          const safeProdUrl = safeUrl(off.product_url);
          const priceDisplay = (off.price_amount && off.price_amount > 0)
            ? `<span class="num-cell"><strong>${escapeHtml(formatPrice(off.price_amount, off.currency))}</strong> <span class="cell-muted">/ ${escapeHtml(off.price_basis || 'unit')}</span></span>`
            : `<span class="badge badge-warning">Perlu minta quotation</span>`;

          return `
            <div class="product-item">
              <div style="display:flex; justify-content:space-between; align-items:flex-start; flex-wrap:wrap; gap:8px;">
                <div style="min-width:0;">
                  <div class="entity-name">${escapeHtml(off.description || 'Produk')}</div>
                  ${safeProdUrl ? `<a href="${escapeHtml(safeProdUrl)}" target="_blank" rel="noopener noreferrer" class="entity-link">Buka halaman produk${EXTERNAL_ICON}</a>` : ''}
                </div>
                <div>${priceDisplay}</div>
              </div>
              <div class="cell-muted num-cell" style="display:flex; flex-wrap:wrap; gap:4px 14px; margin-top:8px;">
                <span>Kecocokan: <strong>${off.product_fit_score.toFixed(0)}%</strong></span>
                <span>Kelengkapan bukti: <strong>${off.evidence_completeness_score.toFixed(0)}%</strong></span>
                ${off.moq ? `<span>MOQ: <strong>${escapeHtml(off.moq)}</strong></span>` : ''}
                ${off.lead_time ? `<span>Lead time: <strong>${escapeHtml(off.lead_time)}</strong></span>` : ''}
              </div>
              ${attrsHtml}
            </div>
          `;
        }).join('');
      }
    }

    // Evidence Provenance Items
    const evContainer = document.getElementById('modal-evidence-list');
    if (evContainer) {
      if (evidenceItems.length === 0) {
        evContainer.innerHTML = `<div class="empty-state"><div class="empty-title">Belum ada kutipan</div><div class="empty-desc">Belum ada teks dari halaman supplier yang dicatat sebagai bukti.</div></div>`;
      } else {
        evContainer.innerHTML = evidenceItems.map(ev => `
          <div class="evidence-box">
            <div style="display:flex; justify-content:space-between; align-items:center; gap:8px; flex-wrap:wrap;">
              <div><span class="cell-muted">Data:</span> <code class="attr-tag" translate="no">${escapeHtml(ev.field_path)}</code></div>
              <span class="badge ${ev.verification_status === 'verified' ? 'badge-success' : 'badge-info'} num-cell">
                <span class="badge-dot"></span>
                <span>${escapeHtml(formatStatusLabel(ev.verification_status))} (${Math.round((ev.confidence || 0) * 100)}%)</span>
              </span>
            </div>
            <div class="evidence-quote">“${escapeHtml(ev.evidence_quote)}”</div>
          </div>
        `).join('');
      }
    }

    // Sources List
    const sourcesContainer = document.getElementById('modal-sources-list');
    if (sourcesContainer) {
      if (sources.length === 0) {
        sourcesContainer.innerHTML = `<div class="empty-state"><div class="empty-title">Belum ada halaman sumber</div><div class="empty-desc">Tidak ada halaman web yang tercatat untuk supplier ini.</div></div>`;
      } else {
        sourcesContainer.innerHTML = sources.map(src => {
          const safeSrcUrl = safeUrl(src.canonical_url);
          return `
            <div class="source-item">
              <div style="display:flex; justify-content:space-between; align-items:flex-start; gap:8px; margin-bottom:4px;">
                <strong style="overflow-wrap:anywhere;">${escapeHtml(src.page_title || 'Halaman web')}</strong>
                <span class="badge badge-neutral">${escapeHtml(src.source_type || 'web')}</span>
              </div>
              ${safeSrcUrl
                ? `<a href="${escapeHtml(safeSrcUrl)}" target="_blank" rel="noopener noreferrer" class="entity-link" translate="no">${escapeHtml(src.canonical_url)}</a>`
                : `<span class="cell-muted" style="overflow-wrap:anywhere;" translate="no">${escapeHtml(src.canonical_url)}</span>`}
              <div class="cell-muted" style="margin-top:4px;">Diambil ${escapeHtml(formatShortTime(src.retrieved_at))} &middot; HTTP ${src.http_status || 200}</div>
            </div>
          `;
        }).join('');
      }
    }

    renderSupplierCorrection(s, data.offers_needing_review || 0);
    renderRfqPanel();

    switchDrawerTab('offers');
    document.getElementById('supplier-modal').classList.add('active');
  } catch (err) {
    showToast('Gagal memuat detail supplier: ' + err.message, 'error');
  }
}

function refreshCurrentView() {
  if (window.location.pathname.endsWith('suppliers.html')) {
    loadSuppliersList();
  } else if (window.location.pathname.endsWith('search.html') && currentJobId) {
    loadJobResults(currentJobId);
  } else if (window.location.pathname.endsWith('index.html') || window.location.pathname === '/') {
    initDashboard();
  }
}

function closeModal() {
  drawerRequestGeneration++;
  const modal = document.getElementById('supplier-modal');
  if (modal) modal.classList.remove('active');
}

// ==========================================
// FORMATTERS & HELPERS
// ==========================================
const EXTERNAL_ICON = '<svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>';

function formatPrice(amount, currency) {
  const num = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 2 }).format(amount);
  return currency ? `${currency} ${num}` : num;
}

function renderStatusBadge(status) {
  const cls = getStatusBadgeClass(status);
  const pulse = (status === 'running' || status === 'queued') ? ' badge-pulse' : '';
  return `<span class="badge ${cls}${pulse}"><span class="badge-dot"></span><span>${escapeHtml(formatStatusLabel(status))}</span></span>`;
}

function getStatusBadgeClass(status) {
  switch ((status || '').toLowerCase()) {
    case 'running':
    case 'discovered':
      return 'badge-info';
    case 'completed':
    case 'approved':
    case 'verified':
    case 'verified_by_purchasing':
      return 'badge-success';
    case 'stopping':
    case 'contacted':
    case 'queued':
      return 'badge-warning';
    case 'stopped':
    case 'failed':
    case 'rejected':
    case 'limit_reached':
      return 'badge-danger';
    default:
      return 'badge-neutral';
  }
}

function formatStatusLabel(status) {
  const map = {
    discovered: 'Ditemukan Web',
    verified_by_purchasing: 'Terverifikasi Purchasing',
    contacted: 'Sudah Dihubungi',
    approved: 'Vendor Disetujui',
    rejected: 'Ditolak',
    queued: 'Dalam Antrean',
    running: 'Berjalan',
    stopping: 'Menghentikan',
    stopped: 'Dihentikan',
    completed: 'Selesai',
    failed: 'Gagal',
    limit_reached: 'Batas Tercapai',
    verified: 'Terverifikasi',
    unverified: 'Belum Diverifikasi',
  };
  const key = (status || '').toLowerCase();
  return map[key] || status || '-';
}

function formatStageLabel(stage) {
  const map = {
    understand: 'Pahami Kebutuhan',
    discover: 'Cari di Web',
    triage: 'Pilih Kandidat',
    extract: 'Ambil Kontak & Info',
    gap_discovery: 'Lengkapi Data',
    completed: 'Selesai',
    stopping: 'Menghentikan',
  };
  const key = (stage || '').toLowerCase();
  return map[key] || stage || '-';
}

function formatSupplierType(type) {
  const map = {
    manufacturer: 'Pabrikan (OEM)',
    authorized_distributor: 'Distributor Resmi',
    distributor: 'Distributor',
    wholesaler: 'Grosir / Trader',
    retailer: 'Retailer',
    marketplace: 'Marketplace',
    unknown: 'Belum Diklasifikasi',
  };
  const key = (type || '').toLowerCase();
  if (map[key]) return map[key];
  return (type || 'Belum Diklasifikasi').replace(/_/g, ' ');
}

function formatCategory(cat) {
  if (!cat) return '-';
  return String(cat)
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

function formatShortTime(ts) {
  if (!ts) return '-';
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) {
    const normalized = String(ts).replace(' ', 'T');
    const d2 = new Date(normalized.endsWith('Z') ? normalized : normalized + 'Z');
    if (Number.isNaN(d2.getTime())) return String(ts);
    return d2.toLocaleString('id-ID', {
      day: '2-digit',
      month: 'short',
      hour: '2-digit',
      minute: '2-digit',
    });
  }
  return d.toLocaleString('id-ID', {
    day: '2-digit',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function getInitials(name) {
  if (!name) return 'SP';
  const parts = String(name).trim().split(/\s+/).filter(Boolean);
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[1][0]).toUpperCase();
}

function safeUrl(url) {
  if (!url) return null;
  try {
    const parsed = new URL(String(url), window.location.origin);
    if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
      return parsed.href;
    }
    return null;
  } catch {
    return null;
  }
}

function escapeHtml(str) {
  if (str === null || str === undefined) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function renderContactValue(container, value, label) {
  container.replaceChildren();
  const text = document.createElement('span');
  text.textContent = value || 'Tidak dipublikasikan';
  container.append(text);
  if (value) {
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'btn btn-secondary btn-xs'; button.textContent = 'Salin';
    button.addEventListener('click', () => copyText(value, label));
    container.append(' ', button);
  }
}

function renderSupplierCorrection(supplier, reviewCount) {
  const body = document.querySelector('#supplier-modal .drawer-body');
  let editor = document.getElementById('supplier-correction');
  if (!editor) { editor = document.createElement('details'); editor.id = 'supplier-correction'; editor.className = 'office-details'; body.prepend(editor); }
  editor.replaceChildren();
  const summary = document.createElement('summary'); summary.textContent = reviewCount ? `Koreksi profil · ${reviewCount} penawaran lama perlu diperiksa` : 'Koreksi profil & lokasi supplier'; editor.append(summary);
  if (reviewCount) {
    const note = document.createElement('p'); note.className = 'field-help';
    note.textContent = `${reviewCount} penawaran lama perlu ditelusuri ulang karena bukti produknya belum diperiksa dengan aturan terbaru. Data tetap tersimpan dan tidak dipakai otomatis sebagai hasil cocok.`;
    editor.append(note);
  }
  const form = document.createElement('form'); form.className = 'office-details-body';
  for (const [field, label] of [['canonical_name','Nama perusahaan'], ['country_code','Kode negara (misalnya ID, CN, SG)'], ['address','Alamat kantor supplier'], ['public_email','Email'], ['public_phone','Telepon / WhatsApp']]) {
    const group = document.createElement('div'); group.className = 'form-group';
    const caption = document.createElement('label'); caption.htmlFor = `correction-${field}`; caption.textContent = label;
    const input = document.createElement('input'); input.id = caption.htmlFor; input.name = field; input.value = supplier[field] || ''; input.maxLength = field === 'country_code' ? 2 : 1000;
    input.required = field === 'canonical_name'; if (field === 'public_email') input.type = 'email';
    group.append(caption, input); form.append(group);
  }
  const save = document.createElement('button'); save.type = 'submit'; save.className = 'btn btn-sm'; save.textContent = 'Simpan koreksi';
  const feedback = document.createElement('p'); feedback.setAttribute('role','status'); feedback.className = 'field-help'; form.append(save, feedback);
  form.addEventListener('submit', async event => {
    event.preventDefault(); save.disabled = true; feedback.textContent = 'Menyimpan…';
    const data = Object.fromEntries(new FormData(form));
    if (!data.country_code.trim()) delete data.country_code; else data.country_code = data.country_code.trim().toUpperCase();
    try { await API.request(`/api/v1/suppliers/${supplier.id}`, { method:'PATCH', body:JSON.stringify(data) }); feedback.textContent = 'Koreksi tersimpan. Profil hasil koreksi dilindungi dari perubahan otomatis.'; refreshCurrentView(); }
    catch (error) { feedback.textContent = error.message; }
    finally { save.disabled = false; }
  });
  editor.append(form);
}

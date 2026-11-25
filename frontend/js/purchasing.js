/* Human-entered quotations are scoped to a search, separately from web offers. */
(() => {
  let job = null;
  let candidates = [];
  let records = [];
  let loading = false;
  let ready = false;
  let pendingRefresh = false;
  let editingVersion = 0;
  let dirty = false;
  let saving = false;
  const labels = { shortlisted: 'Shortlist', contacted: 'Dihubungi', quoted: 'Penawaran diterima', selected: 'Dipilih', rejected: 'Ditolak' };
  const fields = ['status', 'notes', 'pic', 'follow_up_date', 'quotation_ref', 'item', 'quantity', 'unit', 'currency', 'unit_price', 'tax_amount', 'shipping_cost', 'moq', 'lead_time', 'payment_terms', 'valid_until'];
  const numeric = ['quantity', 'unit_price', 'tax_amount', 'shipping_cost'];
  const el = id => document.getElementById(id);
  const name = id => candidates.find(c => c.supplier.id === id)?.supplier.canonical_name || `Supplier #${id}`;
  const esc = value => escapeHtml(String(value ?? ''));
  const amount = (value, currency) => value == null ? 'Belum diisi' : `${esc(currency)} ${new Intl.NumberFormat('id-ID', { maximumFractionDigits: 4 }).format(value)}`;
  function render() {
    el('purchasing-comparison').innerHTML = records.length ? `<table><thead><tr>${['Supplier / status', 'Barang / jumlah', 'Harga satuan', 'Subtotal', 'Total + pajak & ongkir', 'Syarat penawaran', 'PIC / follow-up', 'Catatan / aksi'].map(t => `<th scope="col">${t}</th>`).join('')}</tr></thead><tbody>${records.map(r => {
      const v = r.review;
      const subtotal = v.unit_price != null && v.quantity != null ? v.unit_price * v.quantity : null;
      const total = subtotal != null && v.tax_amount != null && v.shipping_cost != null ? subtotal + v.tax_amount + v.shipping_cost : null;
      const expired = v.valid_until && v.valid_until < `${new Date().getFullYear()}-${String(new Date().getMonth() + 1).padStart(2, '0')}-${String(new Date().getDate()).padStart(2, '0')}`;
      return `<tr><td><strong>${esc(name(r.supplier_id))}</strong><div>${esc(labels[v.status])}</div></td><td>${esc(v.item || 'Belum diisi')}<div>${esc(v.quantity ?? '—')} ${esc(v.unit)}</div></td><td>${amount(v.unit_price, v.currency)}</td><td>${amount(subtotal, v.currency)}</td><td>${amount(total, v.currency)}${total == null ? '<div class="field-help">Lengkapi harga, jumlah, pajak, dan ongkir (isi 0 jika tidak ada).</div>' : ''}</td><td>MOQ: ${esc(v.moq || '—')}<br>Kirim: ${esc(v.lead_time || '—')}<br>Termin: ${esc(v.payment_terms || '—')}<br>Berlaku: ${esc(v.valid_until || '—')}${expired ? '<br><span class="badge badge-warning">Kedaluwarsa</span>' : ''}<br>Ref: ${esc(v.quotation_ref || '—')}</td><td>${esc(v.pic || 'Belum ditentukan')}<br>${esc(v.follow_up_date || 'Belum dijadwalkan')}</td><td><p>${esc(v.notes || '—')}</p><button class="btn btn-secondary btn-sm" type="button" data-edit-review="${r.supplier_id}">Ubah catatan</button><div class="field-help">${esc(r.by)} · ${esc(r.at)} UTC</div></td></tr>`;
    }).join('')}</tbody></table>` : '<div class="empty-state"><div class="empty-title">Belum ada shortlist untuk kebutuhan ini</div><p class="empty-desc">Gunakan tombol “Shortlist / Catat penawaran” pada kandidat supplier di bawah.</p></div>';
    el('purchasing-comparison').querySelectorAll('[data-edit-review]').forEach(button => button.addEventListener('click', () => window.editPurchasingReview(Number(button.dataset.editReview))));
  }
  async function load() {
    if (!job) return;
    if (loading) { pendingRefresh = true; return; }
    loading = true;
    const requestedJob = job;
    try {
      const data = await API.request(`/api/v1/search-jobs/${encodeURIComponent(job)}/purchasing`);
      if (job !== requestedJob) return;
      records = data.reviews;
      ready = true;
      render();
      el('purchasing-history').innerHTML = data.history.map(h => `<p class="review-history-row"><strong>${esc(name(h.supplier_id))}</strong> · ${esc(labels[h.review.status])}<br>${esc(h.review.notes || 'Tanpa catatan')}<br><small>${esc(h.by)} · ${esc(h.at)} UTC</small></p>`).join('') || '<p>Belum ada perubahan.</p>';
    } catch (err) {
      if (job === requestedJob) el('purchasing-comparison').textContent = `Gagal memuat seleksi: ${err.message}. Gunakan Muat ulang untuk mencoba kembali.`;
    } finally { loading = false; if (job !== requestedJob || pendingRefresh) { pendingRefresh = false; await load(); } }
  }
  window.confirmPurchasingLeave = () => {
    if (saving) { showToast('Tunggu sampai penyimpanan selesai.', 'info'); return false; }
    if (dirty && !window.confirm('Ada catatan penawaran belum disimpan. Abaikan dan pindah pencarian?')) return false;
    dirty = false; return true;
  };
  window.setPurchasingCandidates = (jobId, results) => {
    candidates = results;
    if (job === jobId) return;
    job = jobId;
    records = [];
    ready = false;
    el('purchasing-form').hidden = true;
    el('purchasing-panel').hidden = false;
    el('purchasing-comparison').textContent = 'Memuat shortlist…';
    load();
  };
  window.editPurchasingReview = id => {
    if (saving) { showToast('Tunggu sampai penyimpanan selesai.', 'info'); return; }
    if (!ready) { showToast('Shortlist belum termuat. Gunakan Muat ulang pada panel seleksi.', 'info'); return; }
    if (dirty && !window.confirm('Ada perubahan belum disimpan. Abaikan untuk membuka supplier lain?')) return;
    dirty = false;
    const review = records.find(r => r.supplier_id === id)?.review || {};
    editingVersion = review.version || 0;
    el('review-supplier-id').value = id;
    el('quotation-title').textContent = `Seleksi: ${name(id)}`;
    fields.forEach(field => { el(`review-${field}`).value = review[field] ?? (field === 'status' ? 'shortlisted' : ''); });
    el('review-feedback').textContent = '';
    el('purchasing-form').hidden = false;
    el('purchasing-form').scrollIntoView({ block: 'start' });
    el('review-status').focus({ preventScroll: true });
  };
  document.addEventListener('DOMContentLoaded', () => {
    el('refresh-purchasing').addEventListener('click', load);
    el('cancel-review').addEventListener('click', () => { if (dirty && !window.confirm('Abaikan perubahan yang belum disimpan?')) return; dirty = false; el('purchasing-form').hidden = true; });
    el('purchasing-form').addEventListener('input', () => { dirty = true; });
    window.addEventListener('beforeunload', event => { if (dirty) { event.preventDefault(); event.returnValue = ''; } });
    el('purchasing-form').addEventListener('submit', async event => {
      event.preventDefault();
      const payload = Object.fromEntries(fields.map(field => {
        const value = el(`review-${field}`).value.trim();
        return [field, numeric.includes(field) ? (value === '' ? null : Number(value)) : value];
      }));
      payload.version = editingVersion;
      payload.currency = payload.currency.toUpperCase();
      const supplierId = el('review-supplier-id').value;
      const savedJob = job;
      const button = el('save-review');
      saving = true;
      el('purchasing-form').querySelectorAll('input, select, textarea, button').forEach(control => { control.disabled = true; });
      button.disabled = true;
      el('review-feedback').textContent = 'Menyimpan…';
      try {
        const result = await API.request(`/api/v1/search-jobs/${encodeURIComponent(savedJob)}/purchasing/${supplierId}`, { method: 'POST', body: JSON.stringify(payload) });
        if (job === savedJob) {
          editingVersion = result.version; dirty = false;
          el('review-feedback').textContent = 'Tersimpan. Catatan ini khusus kebutuhan/pencarian ini.';
          await load();
        }
      } catch (err) { el('review-feedback').textContent = `Gagal menyimpan: ${err.message}`; }
      finally { saving = false; el('purchasing-form').querySelectorAll('input, select, textarea, button').forEach(control => { control.disabled = false; }); button.disabled = false; }
    });
  });
})();

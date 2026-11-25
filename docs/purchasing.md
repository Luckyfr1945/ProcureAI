# Seleksi dan quotation

Di halaman Cari Supplier, buka hasil pencarian lalu klik **Shortlist / Catat penawaran** pada kandidat. Panel seleksi menyimpan status, catatan keputusan, PIC, tanggal follow-up, dan quotation yang diterima langsung.

- Setiap pencarian memiliki shortlist sendiri. Nomor PR tetap menggunakan referensi kebutuhan pada pencarian; beberapa pencarian dengan nomor PR sama tidak otomatis digabung.
- Satu catatan quotation terkini per supplier per pencarian. Perubahan menyimpan snapshot lengkap dan identitas pengguna dalam transaksi yang sama. UI menampilkan 100 perubahan terbaru.
- Status dipilih/ditolak memerlukan alasan dan tidak mengubah status verifikasi vendor global.
- Harga web tidak otomatis menjadi quotation. Jumlah, satuan, barang, dan mata uang wajib dilengkapi saat mengisi harga satuan.
- Subtotal = harga satuan × jumlah. Total = subtotal + nominal pajak + nominal ongkir. Total ditampilkan hanya setelah seluruh komponen diketahui (isi 0 untuk komponen tanpa biaya).
- Mata uang tidak dikonversi, spesifikasi tidak disetarakan otomatis, dan aplikasi tidak memilih penawaran termurah secara otomatis.
- Pemilik pencarian dan administrator dapat membaca/mengubah seleksi. Pengguna lain tidak memiliki akses ke endpoint seleksi tersebut.
- Tanggal follow-up dicatat; belum ada notifikasi otomatis. Lampiran quotation, approval formal, dan pembuatan PO belum termasuk fitur ini.

Migrasi `0005_purchasing_reviews.sql` diterapkan otomatis ketika aplikasi dimulai ulang. Endpoint terautentikasi: GET `/api/v1/search-jobs/{job_id}/purchasing`, POST `/api/v1/search-jobs/{job_id}/purchasing/{supplier_id}`. Data tersimpan di database aplikasi.

Pembaruan 16 November: catatan quotation memakai `version` untuk mencegah edit usang. Klien harus mengirim versi terakhir; konflik menghasilkan HTTP 409. Detail supplier kini menyediakan koreksi profil/lokasi. Penawaran web lama diberi status perlu tinjauan dan tetap tersimpan; lihat `remediation-2026-11-16.md` untuk dampak migrasi.

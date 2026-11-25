# Perbaikan setelah audit — 16 November 2026

Perubahan diterapkan di source lokal. Tidak mengubah atau me-restart instance aplikasi yang sedang berjalan. Tidak menerapkan migrasi pada database produksi selama pengerjaan.

## Yang diperbaiki

| Audit | Perubahan |
|---|---|
| A01 | Kontak dirender lewat `textContent` dan listener DOM; data tidak lagi menjadi kode di atribut onclick. |
| A02 | Seluruh detail, stop, event, query, hasil, dan ekspor job memeriksa pemilik atau administrator. |
| A03 | Redirect diikuti manual setelah pemeriksaan tiap hop; DNS yang dipakai koneksi dipin, proxy dimatikan, kegagalan DNS ditolak, IPv4-mapped IPv6 privat diblokir. |
| A04 | Secret development dibuat acak per proses; startup production mewajibkan secret unik. Bootstrap admin production memerlukan password yang dikonfigurasi. |
| A05 | Fallback hanya membentuk offer ketika halaman menyebut produk/part yang diminta; brand tanpa penyebutan tidak disalin sebagai fakta. |
| A06 | Identitas produk memakai merek dan nomor part ternormalisasi, tidak hanya nomor part. |
| A07 | Tanpa produk cocok dan bukti produk, skor kecocokan menjadi nol dan worker tidak memasukkannya sebagai hasil cocok. |
| A08 | Pencocokan syarat membandingkan token nama/nilai/satuan; keberadaan key saja tidak mendapat poin. Konversi satuan dan operator numerik lanjutan belum tersedia. |
| A09 | Klaim kontak/lokasi/atribut/harga tanpa dukungan dibuang. Bukti offer dihubungkan dan dibatasi ke produk yang sesuai; harga multi-produk yang ambigu dikosongkan. Verifikasi makna bukti tetap memerlukan manusia. |
| A10–A11 | Status dibatasi; bukti harus milik supplier pada URL; perubahan atomik; supplier tidak ada menghasilkan 404. Verifikasi perusahaan tidak otomatis memverifikasi semua klaim. |
| A12 | Middleware membaca status akun dan role terbaru; logout mencabut token bearer/cookie melalui hash. Token baru memiliki ID unik. Browser memakai cookie HttpOnly tanpa menyimpan bearer di localStorage; mutasi memeriksa Origin. Cookie production memakai Secure. Percobaan login dibatasi dan hashing diverifikasi di thread blocking yang dibatasi. |
| A13 | Enrichment dapat memperbarui profil otomatis; profil yang dikoreksi manusia dilindungi. Form koreksi nama, negara, alamat, email, dan telepon tersedia di drawer supplier. |
| A14 | Platform berisi banyak seller tidak digabung hanya karena domain; nama juga dibandingkan dengan negara/domain. Platform membutuhkan bukti identitas seller. Relasi lama tidak ditebak ulang. |
| A15 | Satu job aktif per worker; task dipantau termasuk panic; stop diperiksa saat menunggu fetch; status stopping dipulihkan setelah restart. |
| A16 | Body diunduh per chunk dengan batas ukuran; timeout menyeluruh per fetch dan jeda request dipakai. |
| A17 | Daftar ISO mencakup 249 kode/nama negara; teks kontak dipertahankan per bagian, domain bukan pengganti alamat fisik. Negara/kota target tetap menjadi filter. Cabang dan nomor layanan lintas negara masih membutuhkan pemeriksaan manusia. |
| A18 | Query mencakup kota-kota pilihan, sinonim dan variasi supplier/katalog; alokasi query mencampur rencana AI dan fallback. |
| A19 | Respons status/log/query/hasil usang diabaikan; polling tidak bertumpuk. Drawer memiliki generation ID agar respons supplier lama tidak menimpa supplier baru. |
| A20 | Keyword direktori dicari di server, dengan pagination 50 supplier; supplier di luar 100 entri pertama dapat ditemukan. |
| A21 | Drawer dari pencarian memakai offers untuk job tersebut. RFQ dari direktori meminta pengguna menentukan barang dan tidak memakai offer pertama secara diam-diam. |
| A22 | Quotation memakai optimistic version; edit usang ditolak HTTP 409. Refresh diantrikan; formulir melindungi perubahan belum disimpan dan mencegah pergantian saat save berjalan. |
| A23 | Statistik job mengikuti pengguna, error dashboard ditampilkan, rank hasil dihitung ulang setelah filter, jumlah kandidat layar mengikuti hasil aktual, label request tidak lagi menyebut putaran. |

## Perlakuan data lama

Migrasi `0007_offer_provenance_review.sql` menandai **seluruh penawaran yang sudah ada sebelum migrasi** sebagai `needs_review=1`. Ini sengaja konservatif karena bug ekstraksi dan penggabungan merek dapat mencemari data lama. Baris asli tetap disimpan; harga, quotation manusia, dan riwayat seleksi tidak dihapus.

Penawaran tersebut tidak dipakai otomatis dalam pencarian atau sebagai offer aktif. Detail supplier menampilkan jumlah data yang perlu ditinjau. Jalankan penelusuran baru untuk memperoleh offer berbukti dengan aturan baru. Koreksi profil perusahaan melalui drawer jika lokasi/kontaknya salah. Penggabungan produk/perusahaan lama yang salah tidak dipisahkan otomatis karena identitas aslinya belum pasti.

## Validasi

- `cargo test`: 32 tes Rust, meliputi akses seluruh endpoint job, token setelah logout, akun nonaktif, merek berbeda dengan part sama, fallback halaman tidak relevan, bukti vendor lain, pencarian supplier di luar 100 entri pertama, koreksi profil atomik, karantina offer lama, negara khusus, dan konflik versi quotation.
- `node tests/frontend_regressions.cjs`: 3 tes JavaScript untuk keamanan render kontak, respons status usang, dan penghapusan token dari localStorage.
- `node --check frontend/js/app.js` dan `node --check frontend/js/purchasing.js`.
- `git diff --check`.
- Uji browser, pengukuran beban, serta akurasi pencarian provider/AI langsung belum dilakukan. Tes worker yang menggunakan provider mock memeriksa lifecycle, bukan menjamin kualitas hasil internet.

## Penggunaan setelah pembaruan

Restart backend untuk menjalankan migrasi `0006` dan `0007`. Development tanpa SESSION_SECRET tetap akan meminta login ulang setelah restart karena secret dibuat acak; gunakan SESSION_SECRET yang dikonfigurasi jika sesi perlu bertahan. Production wajib memakai secret kuat dan password bootstrap yang sesuai ketentuan ketika membuat admin pertama.

Perubahan version quotation membutuhkan frontend dan backend versi yang sama. Klien API lain harus mengirim `version` dari respons daftar seleksi saat mengubah catatan; nilai 0 digunakan untuk catatan baru.

## Batas yang masih ada

Aplikasi tetap alat riset dan seleksi supplier. Kebenaran klaim tidak dapat ditentukan hanya dari kemunculan teks. Cabang multi-negara, nomor hotline internasional, sinonim teknis yang kompleks, operator toleransi/konversi satuan, identitas seller yang tidak dipublikasikan, serta dokumen PDF/situs JavaScript masih memerlukan penanganan lebih lanjut.

Lampiran quotation, approval, PO, ERP, reminder otomatis, dan workflow banyak item per PR belum ditambahkan dalam perbaikan bug ini. Observabilitas operasional, uji browser dan beban, serta rate limit terdistribusi masih perlu dilengkapi sebelum penggunaan produksi yang luas. Pembatasan percobaan login saat ini berlaku per proses dan per nama akun; deployment beberapa instance membutuhkan penyimpanan rate limit bersama.

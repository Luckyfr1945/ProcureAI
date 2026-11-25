# ProcureAI

Aplikasi web untuk bantu tim purchasing cari calon supplier di internet.

Idenya dari pengalaman waktu PKL. Cari supplier itu makan waktu: buka Google, buka satu-satu website, cari ada barangnya atau tidak, catat kontak dan harga, lalu ulangi lagi buat barang lain. ProcureAI mencoba mengotomatiskan bagian itu. Kita cukup tulis barang yang dicari, aplikasi yang mencari, membuka halaman supplier, dan mengumpulkan datanya.

Hasilnya tetap harus dicek manual. Aplikasi ini alat bantu riset, bukan pengganti keputusan purchasing. Karena itu setiap data supplier disimpan bersama link halaman sumbernya, jadi gampang dicek ulang.

## Fitur

- Cari supplier dari deskripsi barang (nama, merek, kode part, spesifikasi, lokasi)
- Daftar supplier dengan skor kecocokan produk dan kelengkapan data
- Detail supplier: produk, kontak, alamat, dan kutipan bukti dari halamannya
- Direktori semua supplier yang pernah ditemukan, bisa dikoreksi manual
- Shortlist dan catatan quotation per pencarian (status, PIC, follow-up, harga)
- Ekspor hasil pencarian ke Excel
- Login dengan role admin dan purchasing

## Cara kerjanya

1. Kebutuhan barang dianalisis oleh Gemini (atau heuristik kalau Gemini tidak tersedia).
2. Dibuat beberapa variasi kata kunci, lalu dicari lewat search engine.
3. Hasil pencarian disaring, lalu halaman supplier dibuka satu per satu.
4. Dari tiap halaman diambil nama perusahaan, produk, kontak, dan lokasi.
5. Supplier yang sama digabung, yang di luar lokasi target dibuang, sisanya diurutkan.
6. Kalau hasilnya masih sedikit, aplikasi mencari lagi dengan kata kunci tambahan.

Satu pencarian biasanya selesai dalam 1 sampai 3 menit.

## Teknologi

- Backend: Rust, Axum, Tokio
- Database: SQLite (SQLx, migrasi otomatis saat start)
- Frontend: HTML, CSS, JavaScript biasa
- AI: Google Gemini API
- Search: SearXNG (self-hosted, gratis)
- Crawling: reqwest + scraper

## Menjalankan di lokal

Yang perlu ada: Rust (versi terbaru, project ini pakai edition 2024) dan Podman atau Docker.

### 1. Jalankan SearXNG

```bash
mkdir -p ~/searxng
podman run -d --name searxng -p 127.0.0.1:8888:8080 \
  -v ~/searxng:/etc/searxng:Z docker.io/searxng/searxng:latest
```

Buka `~/searxng/settings.yml`, tambahkan ini di bagian bawah supaya SearXNG bisa mengembalikan JSON:

```yaml
search:
  formats:
    - html
    - json
```

Lalu restart dengan `podman restart searxng`. Cek dengan:

```bash
curl "http://127.0.0.1:8888/search?q=test&format=json"
```

Kalau file `settings.yml` tidak bisa diedit karena permission, pakai `podman unshare nano ~/searxng/settings.yml`.

Setelah komputer restart, nyalakan lagi dengan `podman start searxng`.

### 2. Atur `.env`

```bash
cp .env.example .env
```

Yang perlu diubah:

```
SEARCH_PROVIDER=searxng
SEARCH_API_BASE_URL=http://127.0.0.1:8888/search
GEMINI_API_KEY=isi_api_key_gemini
SESSION_SECRET=isi_dengan_string_acak_panjang
DEFAULT_ADMIN_PASSWORD=password_admin_kamu
```

- `GEMINI_API_KEY` bisa dibuat gratis di Google AI Studio. Tanpa key ini aplikasi tetap jalan, tapi hasil ekstraksinya kurang bagus.
- `SESSION_SECRET` bisa dibuat dengan `openssl rand -hex 32`.
- `DEFAULT_ADMIN_PASSWORD` dipakai untuk akun `admin` yang dibuat otomatis saat aplikasi pertama kali jalan. Isi sebelum menjalankan pertama kali.

### 3. Jalankan

```bash
cargo run
```

Buka http://127.0.0.1:8080, login sebagai `admin`.

Kalau sebelumnya pernah menjalankan versi lama dan muncul error migrasi, hapus `instance/procure_ai.db` lalu jalankan lagi.

## Tes

```bash
cargo test
node tests/frontend_regressions.cjs
```

## Struktur folder

```
src/
  api/          endpoint REST
  auth/         login, session, middleware
  database/     koneksi dan query SQLite
  domain/       struct data utama
  providers/    adapter search engine
  services/     crawler, Gemini, lokasi, ranking, dll
  workers/      proses pencarian di background
frontend/       halaman web
migrations/     skema database
tests/          tes integrasi
docs/           catatan audit dan fitur purchasing
```

## Keterbatasan

- Data diambil dari teks halaman web, jadi bisa salah. Selalu cek link sumbernya.
- Belum bisa membaca PDF, katalog, atau website yang isinya dimuat lewat JavaScript.
- Harga dari web belum tentu masih berlaku.
- Pencarian dengan kode part saja (tanpa nama barang atau merek) sering tidak dapat hasil. Tulis nama barangnya juga.
- Kalau kuota Gemini habis, aplikasi pakai heuristik dan nama perusahaan kadang terambil dari judul halaman.
- Belum ada lampiran quotation, approval, atau pembuatan PO.

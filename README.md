# DiskTree (DiskAğacı)

Düşük bellekli çevrimdışı disk analizörü: sabit bellekli dizin taraması, **squarified
treemap**, **ısı haritası** ve **boyut zaman çizelgesi**. Tek dosya dağıtımı, yönetici
yetkisi gerektirmez, hiçbir ağ bağlantısı kurmaz ve **hiçbir dosyayı silmez,
taşımaz veya değiştirmez**.

- **Kategori:** Sistem ve Disk
- **Rapor:** `%USERPROFILE%\Desktop\Fikirler\06-disk-agaci-analizor.html` (06/30)
- **Lisans:** MIT
- **MSRV (rust-version):** 1.74
- **Bağımlılıklar:** `serde` (derive), `serde_json`, `clap` (derive) — üçü de doğrudan

---

## Rapor sapması — neden MFT okumuyoruz

Bu bölüm bilinçlidir ve zorunludur.

Rapor (06/30, `b07` ve `b05`) NTFS **Master File Table** kayıtlarını doğrudan okumayı
önerir. Gerekçesi doğrudur: NTFS'te bütün dosya sistemi disk üzerinde zaten indeksli
bir ağaçtır, dolayısıyla her dosya için ayrı dizin sorgusu yapmaya gerek yoktur.

**Bu MVP'de MFT okunmaz; birincil mod dizin ağacı yürüyüşüdür
(`std::fs::read_dir`).** Gerekçe iki maddeden oluşur:

1. **Yönetici yetkisi ve ham disk erişimi.** `\\.\C:` yolunu açmak Windows'un çoğu
   kurulumunda yönetici yetkisi ister. Raporun kendi kabul kriteri (S1) "yönetici
   olmadan sonuç üret" der ve `b10` bölümü "varsayılan olarak yönetici yetkisi
   istemez" diye yazar. Bu iki ifade, ham birim açmayı aynı rapor içinde tutarsız
   kılar. Ayrıca BitLocker'lı birimde MFT okuma sessizce **"boş"** sonuç üretir
   (raporun R2 riski) — yani yönetici modu doğru sonucu değil, **yanlış** sonucu
   üretebilir.
2. **Portable ve kurulum gerektirmeyen ilke.** Bu orkestrasyonun ortap taahhüdü:
   "kurulum gerektirmeyen, tek dosya dağıtılabilen, program dizini dışına
   yazmayan" araç. Yönetici yetkisi isteyen bir tarama, kullanıcının her seferinde
   UAC onayı vermesini ve "yönetici olarak çalıştır" hatasını görmesini
   gerektirir; bu, "portable" sözünün karşısındadır.

**Ne korundu:** Rapordaki en ayırt edici mimari kararı, yani **sabit bellekli gezinme**,
birebir uygulandı. Dizin yürüyüşünün sonucu da MFT'çi bir yol/index tablosu
türetir; fark yalnızca kaynağın dizin girişlerinden okunmasıdır. Bellek bütçesi,
`array of structs` + kimlik veri modeli ve "işaretçi tablosu tutulmaz" kararı
(`b07`) aynen korunmuştur.

**Ne kaybedildi (dürüstlük):**

- Birimdeki **tüm** kayıtlar görülmez. Silinmiş ama henüz geri alınmamış NTFS
  kayıtları, `$Recycle.Bin` içeriği ve dizin indeksinde yer almayan gizli
  akışlar rapora girmez.
- `alternatif veri akışları` (NTFS ADS) taranmaz.
- Ayırma denetimindeki "kayıp + küme" değeri **ölçüm değil tahmindir** (bkz.
  [Bilinen Sınırlamalar](#bilinen-sınırlamalar)).
- "Yönetici gerektirmeyen modun sınırı" belgelenmiştir: yönetici olmayan bir
  oturumda `System Volume Information` gibi yollar atlanır ve **sayıları ayrı
  raporlanır** (`erisim_reddi` sayacı).

Rapordaki ayrıca ertelenmiş olan ext4/APFS ham okuyucuları da bu MVP'de yoktur;
`MANIFEST.md` kartında da ertelenmişler arasındadır.

---

## Özellikler

- **Sabit bellekli tarama** — `std::fs::read_dir` ile özyinelemeli yürüyüş. Tam ağaç
  hiçbir zaman belleğe alınmaz; yol dizesi havuzunda `(ofset, uzunluk)` çiftiyle
  saklanır, çocuk listesi `ilk_cocuk` / `sonraki_kardes` indeks çiftiyle kurulur
  (düğüm başına `Vec` ayırma yoktur).
- **Sonsuz döngü koruması** — her dizin **kanonik** yoluyla kilitlenir; sembolik
  bağ ve `..` çözümlendikten sonra aynı dizine ikinci giriş sayılır ve engellenir.
  Sembolik bağlar varsayılan olarak hiç izlenmez.
- **İzin hataları taramayı durdurmaz** — her `read_dir`/`metadata` hatası sayılır,
  kayda geçirilir ve tarama devam eder. Yalnızca **kök** okunamazsa hata döner.
- **Squarified treemap** — Bruls / Huizing / van Wijk algoritması, kendi yazımımız.
  Her kutunun alanı, değerinin toplam alandaki oranıyla **birebir** aynıdır;
  kutular kesişmez, dikdörtgenin içinde kalır, en-boy oranı kareye yakın kalır.
- **Terminal ASCII treemap** — renksiz ızgara çıktısı. Karakter, kutunun alan
  sıralamasını kodlar; böylece kutuların sınırları görsel olarak ayrılır.
- **Boyut zaman çizelgesi** — aylık kümülatif büyüme eğrisi (dosya son değişiklik
  tarihine göre) ve iki taramayı yol bazlı karşılaştıran "hangi klasör ne zaman
  büyüdü" listesi (`buyudu` / `kuculdu` / `yeni` / `silinmis`).
- **Isı haritası verisi** — değişim büyüklüğü × derinlik skoru
  (`1 / (1 + derinlik)`), 0-1 normalize edilmiş ve 0-4 yoğunluk seviyesine çevrilir.
- **Büyük / eski / şüpheli gizli dosya listesi** — eşik tabanlı, boyut ve yaşa
  göre sıralı, her satırda gerekçe metni. Arşiv uzantıları varsayılan olarak
  dışlanır.
- **Siyah kutu (kayıp + küme) alan tahmini** — `kapasite = dosya verisi + yedeklenmiş
  kümeler + boş küme + kayıp` denkleminin hesaplanan satırı ve **her terimin nereden
  geldiğini anlatan açıklaması**.
- **Anlık görüntü (JSON önbellek)** — tarama sonucu `disktree-kayma/1` biçiminde
  diske yazılır; ikinci çalıştırmada yeniden okunur. Yazma **atomiktir** (geçici
  dosya → `rename`).
- **Yedi alt komut:** `scan`, `tree`, `treemap`, `timeline`, `heatmap`, `bigfiles`,
  `report`.
- **Sıfır yazma garantisi** — hiçbir komut taranan ağaca yazmaz, dosya silmez,
  taşımaz veya izin değiştirmez. Bu, `tests/entegrasyon.rs` içindeki
  `hicbir_komut_dosya_silmez_ya_da_degistirmez` testiyle kanıtlanmıştır.
- **Sıfır ağ** — `Cargo.toml`da hiçbir ağ crate'i yoktur; ikili hiçbir URL'ye
  istek yapmaz.
- **`#![forbid(unsafe_code)]`** — kaynak kodun tamamında `unsafe` yasaktır.

---

## Kurulum

### Gereksinimler

| Bileşen | Sürüm |
|---|---|
| Rust (MSRV) | 1.74 veya üzeri |
| Doğrulama ortamı | `cargo 1.98.1` · `rustc 1.98.1` (Windows, `x86_64-pc-windows-msvc` görünümünde `gnu` araç zinciri ile) |
| Bağımlı dış kütüphane | **Yok** (yalnızca işletim sistemi kitaplıkları) |

### Kaynaktan derleme

```console
$ cargo build --release
   Compiling disktree v0.1.0 (%USERPROFILE%\Desktop\Projeler\projects\06-disktree)
    Finished `release` profile [optimized] target(s) in 27.96s
```

Çıktı: `target\release\disktree.exe` (1 296 910 bayt).

### Kurulum (tek dosya)

```console
$ cargo install --path .
      Installing %USERPROFILE%\.cargo\bin\disktree.exe
   Installed package `disktree v0.1.0 (%USERPROFILE%\Desktop\Projeler\projects\06-disktree)` (executable `disktree.exe`)
```

Doğrulama:

```console
$ disktree --version
disktree 0.1.0
```

### Demo ağacını üret (README örnekleri için)

```console
$ powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\demo-olustur.ps1
demo agaci hazir: 12 dosya, 24931041 bayt -> %USERPROFILE%\Desktop\Projeler\projects\06-disktree\demo
```

Bu komut 12 dosyalık, 24 931 041 baytlık sahte bir ağaç üretir. İçeriği tamamen
sabit tohumlu rastgele baytlardır; **hiçbir gerçek kullanıcı verisi kullanılmaz ve
ağdan hiçbir şey indirilmez.** `demo/` klasörü `.gitignore` içindedir.

---

## Kullanım

Aşağıdaki komutların **tamamı** yukarıdaki demo ağacı üzerinde gerçekten
çalıştırılmış, çıktılar kopyalanmıştır.

### 1) Tarama ve anlık görüntü

```console
$ disktree scan demo --cikti onceki.json
demo  dugum=22 dosya=12 dizin=10 toplam=23.8 MiB bellek=2.7 KiB  [dizin=10 atlanan=0 erisim_reddi=0 dongu=0 derinlik_asim=0 haric=0 bag=0 kisaltildi=false tamamlandi=true sure=0.002sn]
anlik goruntu: onceki.json
```

Özet satırındaki sayaçlar: `dizin` = açılıp okunan dizin, `atlanan` = özniteliği
okunamayan girdi, `erisim_reddi` = izin/yol hatası, `dongu` = döngü korumasının
devreye girdiği sayı, `derinlik_asim` = derinlik sınırına takılan dizin,
`haric` = kullanıcı dışlaması, `bag` = sembolik bağ. `bellek` sütunu, taranan
ağacın bellekte kapladığı toplam baytıdır (yol havuzu + kayıt dizisi) — dosya
içerikleri hiçbir zaman belleğe alınmaz.

### 2) Metin ağaç çıktısı

```console
$ disktree tree demo --derinlik 3 --ust-puan 5
demo  dugum=22 dosya=12 dizin=10 toplam=23.8 MiB bellek=2.7 KiB  [dizin=10 atlanan=0 erisim_reddi=0 dongu=0 derinlik_asim=0 haric=0 bag=0 kisaltildi=false tamamlandi=true sure=0.002sn]
   |-- projeler                    8.1 MiB  #######.............  33.9%  d1
      |-- gorsel                      6.7 MiB  #################...  82.8%  d2
         |-- buyuk-video.mp4             5.8 MiB  #################...  87.1%  d3
         `-- tanitim.fla               878.9 KiB  ###.................  12.9%  d3
      `-- ses                         1.4 MiB  ###.................  17.2%  d2
         `-- podcast-son-3.mp3           1.4 MiB  #################### 100.0%  d3
   |-- film                        7.0 MiB  ######..............  29.3%  d1
      `-- tam-izleme.mkv              7.0 MiB  #################### 100.0%  d2
   |-- muzik                       6.2 MiB  #####...............  26.2%  d1
      |-- canli                       5.9 MiB  ###################.  95.1%  d2
         |-- 01-parca.flac               3.6 MiB  ############........  61.3%  d3
         `-- 02-parca.flac               2.3 MiB  ########............  38.7%  d3
      `-- albüm-kapak.jpg           312.5 KiB  #...................   4.9%  d2
   |-- belgeler                    2.5 MiB  ##..................  10.7%  d1
      |-- eski                        2.0 MiB  ################....  80.5%  d2
         |-- arsiv-2023.zip              2.0 MiB  ####################  98.1%  d3
         `-- notlar.txt                 40.0 KiB  ....................   1.9%  d3
      |-- rapor.pdf                 507.8 KiB  ####................  19.5%  d2
      `-- okunacaklar.md                 41 B  ....................   0.0%  d2
   `-- arsivler                        0 B  ....................   0.0%  d1
   ... 1 cocuk daha
```

Türkçe dosya adı (`albüm-kapak.jpg`) sorunsuz okunur: yol havuzu UTF-8 saklar ve
JSON çıktısı bu adı bozmadan yazar.

### 3) Treemap (ASCII ızgara)

```console
$ disktree treemap demo --genislik 64 --yukseklik 18 --en-cok 10
demo  dugum=22 dosya=12 dizin=10 toplam=23.8 MiB bellek=2.7 KiB  [dizin=10 atlanan=0 erisim_reddi=0 dongu=0 derinlik_asim=0 haric=0 bag=0 kisaltildi=false tamamlandi=true sure=0.002sn]
treemap: demo  toplam=23.8 MiB  cocuk=6
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......
@@@@@@@@@@@@@@@@@@@@@@##################=================.......

rampa: bos=' ' (kucuk) ... '@' (buyuk); 4 kutu, 23.8 MiB toplam, izgara 64x18
```

`@` en büyük kutu (`projeler`), `=` üçüncü büyük, `.` en küçük kutu. Kutu sayısı
6 değil **4** çünkü `arsivler/` (boş dizin) ve `bos-dosya.bin` (0 bayt) toplamı 0
olan girdilerdir; sıfır alanlı kutular yerleşime alınmaz.

Kutuların JSON hâli:

```console
$ disktree treemap demo --genislik 20 --yukseklik 10 --en-cok 4 --cikti treemap.json
rampa: bos=' ' (kucuk) ... '@' (buyuk); 4 kutu, 33.1 MiB toplam, izgara 20x10
treemap json: treemap.json
```

```json
[
  {
    "etiket": "projeler",
    "deger": 16750000,
    "x": 0.0,
    "y": 0.0,
    "genislik": 9.656950246902273,
    "yukseklik": 10.0
  },
  {
    "etiket": "muzik",
    "deger": 8020000,
    "x": 9.656950246902273,
    "y": 0.0,
    "genislik": 8.832506136271215,
    "yukseklik": 5.234986945169712
  },
  {
    "etiket": "film",
    "deger": 7300000,
    "x": 9.656950246902273,
    "y": 5.234986945169712,
    "genislik": 8.832506136271215,
    "yukseklik": 4.765013054830288
  },
  {
    "etiket": "belgeler",
    "deger": 2620041,
    "x": 18.48945638317349,
    "y": 0.0,
    "genislik": 1.5105436168265118,
    "yukseklik": 10.0
  }
]
```

**Alan değişmezi elle doğrulanabilir.** Dört kutunun değer toplamı
`16 750 000 + 8 020 000 + 7 300 000 + 2 620 041 = 34 690 041` bayttır ve ızgara
alanı `20 × 10 = 200` birimdir. `projeler` kutusu `16 750 000 / 34 690 041 × 200
= 96.57` birim alan kaplamalıdır; JSON'daki `9.6569… × 10.0 = 96.5695` tam olarak
bunu verir. Aynı denetim `treemap::tests::kutu_alani_girdi_toplami_ile_orantili_durur`
ve `treemap_alani_girdi_toplamiyla_ortusur` testlerinde otomatik olarak yapılır.

### 4) Zaman çizelgesi

İkinci tarama için demo ağacı değiştirilir (üç yeni dosya, bir silinen dosya):

```console
$ disktree scan demo --cikti sonraki.json --sessiz
$ disktree timeline onceki.json sonraki.json --en-cok 5 --aylik
aylik kumulatif buyume egrisi (demo):
  2026-09    34690041 bayt    14 dosya  kumulatif     34690041

boyut farki: 11 satir (21 kayit karsilastirildi)
  1. buyudu              +8300000 bayt     6.7 MiB -> 14.6 MiB   demo\projeler\gorsel
  2. buyudu              +8300000 bayt     8.1 MiB -> 16.0 MiB   demo\projeler
  3. yeni                +5200000 bayt         0 B -> 5.0 MiB    demo\projeler\gorsel\yeni-cekim.mov
  4. yeni                +3100000 bayt         0 B -> 3.0 MiB    demo\projeler\gorsel\ham\kare-0001.dng
  5. yeni                +3100000 bayt         0 B -> 3.0 MiB    demo\projeler\gorsel\ham
```

(Demo ağacı tek günde üretildiği için yalnızca `2026-09` kovası vardır; gerçek
kullanımda kova sayısı dosya sayısına yakındır.)

### 5) Isı haritası

```console
$ disktree heatmap onceki.json sonraki.json --en-cok 5
isi haritasi: 11 degisen yol (derinlik 1 = en sicak)
*             8300000 bayt  derinlik 1   skor   4150000.00  demo\projeler
+             8300000 bayt  derinlik 2   skor   2766666.67  demo\projeler\gorsel
:             5200000 bayt  derinlik 3   skor   1300000.00  demo\projeler\gorsel\yeni-cekim.mov
:             3100000 bayt  derinlik 3   skor    775000.00  demo\projeler\gorsel\ham
:             1500000 bayt  derinlik 1   skor    750000.00  demo\muzik
```

İlk iki satır aynı değişimi (8 300 000 bayt) gösterir ama farklı derinlikte
oldukları için farklı skorlanır: sığ olan daha "sıcak"tır
(`8 300 000 / 2 = 4 150 000` ve `8 300 000 / 3 = 2 766 666.67`).

### 6) Büyük dosya listesi

```console
$ disktree bigfiles demo --esik-bayt 1000000 --yas-gun 0 --en-cok 5
demo  dugum=25 dosya=14 dizin=11 toplam=33.1 MiB bellek=2.8 KiB  [dizin=11 atlanan=0 erisim_reddi=0 dongu=0 derinlik_asim=0 haric=0 bag=0 kisaltildi=false tamamlandi=true sure=0.003sn]
buyuk dosya listesi: esik 976.6 KiB , yas 0 gun , 5 satir
   7.0 MiB      0g   demo\film\tam-izleme.mkv
   5.8 MiB      0g   demo\projeler\gorsel\buyuk-video.mp4
   5.0 MiB      0g   demo\projeler\gorsel\yeni-cekim.mov
   3.6 MiB      0g   demo\muzik\canli\01-parca.flac
   3.0 MiB      0g   demo\projeler\gorsel\ham\kare-0001.dng
not: bu liste yalnizca gozlem sunar; arac hicbir dosyayi silmez veya degistirmez.
```

`demo\belgeler\eski\arsiv-2023.zip` (2.0 MiB) listede **yoktur**: arşiv
uzantıları varsayılan olarak dışlanır. `--arsivleri-dahil-et` ile listeye girer.

### 7) Ayırma denetimi ("siyah kutu")

```console
$ disktree report demo --kapasite 500107862016 --bos 220000000000 --kume 4096 --cikti rapor.json
demo  dugum=25 dosya=14 dizin=11 toplam=33.1 MiB bellek=2.8 KiB  [dizin=11 atlanan=0 erisim_reddi=0 dongu=0 derinlik_asim=0 haric=0 bag=0 kisaltildi=false tamamlandi=true sure=0.002sn]
ayirma (siyah kutu): dosya_verisi = 14 dosyanin mantiksal bayt toplami (34690041) ; kume_isaret = 27655 bayt (kume 4096 bayt varsayimiyla her dosyanin bos kalan son kume parcasi) ; tahmini_kullanim = dosya_verisi + kume_isaret ; yedeklenmis ve bos degerleri NTFS denetim bilgisi okunmadigi icin disaridan verilmistir ; kayip = kapasite - (tahmini_kullanim + yedeklenmis + bos). Bu bir tahmindir, olcum degildir: MFT, $Bitmap ve muf/start alanlari okunmamistir, yalnizca dizin yuruyusu kullanilmistir.
kayip: 280073144320 bayt (tahmin, olcum degildir)
rapor: rapor.json
```

Açıklamada sayılan hesap: `34 690 041 + 27 655 = 34 717 696` tahmini kullanım;
`500 107 862 016 − (34 717 696 + 0 + 220 000 000 000) = 280 073 144 320` bayt
kayıp.

### Tüm komutların yardımı

```console
$ disktree --help
Düşük bellekli çevrimdışı disk analizörü: sabit bellekli tarama, squarified treemap, ısı haritası ve boyut zaman çizelgesi.

Usage: disktree.exe <COMMAND>

Commands:
  scan      Dizin ağacını tara ve anlık görüntüyü JSON olarak kaydet
  tree      Metin ağaç çıktısı üret (boyut çubuklu)
  treemap   Squarified treemap üret (ASCII ızgara + isteğe bağlı JSON)
  timeline  İki anlık görüntü arasındaki boyut farkını göster
  heatmap   İki anlık görüntüden ısı haritası verisi üret
  bigfiles  Büyük, eski ve şüpheli gizli dosya listesi
  report    Tüm çıktıları tek bir JSON raporunda birleştir
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

---

## Test

```console
$ cargo test
     Running unittests src\lib.rs (target\debug\deps\disktree-27fec6eaeff6d1e9.exe)

running 118 tests
test result: ok. 118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running unittests src\main.rs (target\debug\deps\disktree-afb1670f78e2a70d.exe)

running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\entegrasyon.rs (target\debug\deps\entegrasyon-3c5136e985d27605.exe)

running 26 tests
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s

   Doc-tests disktree

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**test sonucu: okunan 148; başarısız 0** (118 birim + 4 CLI birimi + 26 entegrasyon).

Kapsanan uç durumlar (her biri gerçek bir `#[test]` fonksiyonudur):

| Alan | Testler |
|---|---|
| Boş klasör | `bos_klasor_taranir_ve_tek_dugum_uretir`, `bos_agac_sifir_veri_uretir` |
| Tek dosya | `tek_dosya_taranir` |
| İç içe ağaç | `ic_ice_agac_toplamlari_hesaplar`, `uctan_uca_tarama_toplamlari_dogru_hesaplar`, `taranan_agac_turleri_dogru_siniflandirilir` |
| Gizli dosya | `gizli_dosya_isareti_okunur_ve_istenince_atlanir`, `gizli_dosyalar_istenince_dahil_edilir`, `gizli_dosya_tarama_icerisinde_sayilir` |
| Sembolik bağ döngüsü | `dongu_koruma_ayni_yolu_iki_kez_reddeder`, `dongu_koruma_tohumlama_dizine_girmeyi_engeller`, `dongu_koruma_ayni_dizine_ikinci_girisi_reddeder` (uçtan uca), `sembolik_bag_izleme_acikken_kanonik_yolla_korunur`, `sembolik_bag_dongusu_tarama_terminasyonu_korur` |
| İzin hatası atlanıyor | `izin_hatasi_taramayi_durdurmaz_ve_kok_hatasi_doner`, `izin_hatasi_tarama_ozetinde_sayilir_ve_agaci_bozmaz`, `olmayan_kok_hata_dondurur` |
| Kısmi tarama | `kismi_tarama_maks_dugum_sinirinda_durur`, `kismi_tarama_dugum_siniriyla_kisaltilir` |
| Treemap alan toplamı = girdi toplamı | `kutu_alani_girdi_toplami_ile_orantili_durur`, `treemap_alani_girdi_toplamiyla_ortusur`, `tek_kutu_tum_alani_kaplar` |
| Treemap dikdörtgen oranı | `en_boy_orani_kareye_yakin_kalir`, `kutular_birbiriyle_kesismez`, `kutular_dikdortgenin_icinde_kalir` |
| Aşırı derinlik sınırı | `asiri_derinlik_siniri_uygulanir`, `derinlik_siniri_icer_noktalari_gormesin` |
| Büyük / sıfır boyutlu dosya | `sifir_boyutlu_dosya_sayilir_ama_toplami_artirmaz`, `sifir_boyutlu_dosya_toplami_bozmaz_ama_sayilir`, `buyuk_dosyalar_esige_gore_suzulur`, `sifir_boyutlu_dosya_kume_isaretine_katilmaz` |
| Zaman çizelgesi sıralaması | `fark_buyuyen_klasoru_ilk_siraya_koyar`, `fark_artan_ve_kucunen_yollari_degistirilmeden_ayirir`, `aylik_kume_kumulatif_toplam_verir`, `aylik_kume_yalnizca_dosyalari_sayar` |
| Takvim dönüşümü | `bilinen_tarihler_dogru_gun_sayisi_verir`, `gun_sayisi_ve_tarih_arasindaki_gidis_donusu_kayipsizdir`, `gun_sinirinda_ay_degismez_ve_ay_sinirinda_degisir`, `mart_2000_ve_eylul_2026_ay_etiketleri_dogru` |
| Tahmin (kayıp + küme) | `kayip_kapasite_eksi_bilinen_terimlerdir`, `kume_isareti_kismi_kumeleri_toplar`, `negatif_kayip_uyari_uretir`, `kume_boyutu_sifir_kenari_ayirma_denetimi_hata_dondurur` |
| `--dry-run` karşılığı | `cli_dry_run_kipi_yoktur_ama_sessiz_bayragi_calisir` |
| Arşiv dışlama | `arsiv_dosyalari_varsayilan_olarak_dislanir`, `arsiv_uzantisi_tespiti_buyuk_kucuk_harf_duyarsizdir`, `buyuk_dosya_listesi_arsivleri_disarida_birakir` |
| Dışlama kalıbı | `haric_tutulan_kalibi_girdileri_atlar`, `haric_tutulan_uzantiya_gore_calisir`, `kalip_uyum_alt_dizi_ve_yildiz_onesiyle_calisir` |
| JSON şema gidiş-dönüşü | `kayma_json_gidis_donusu_kayipsizdir`, `kayma_yazma_ve_okuma_dosya_uzerinden_calisir`, `json_sema_gidis_donusu_kayipsizdir`, `bozuk_json_hata_dondurur`, `yanlis_bicim_imzasi_reddedilir`, `yanlis_surum_reddedilir` |
| ASCII çıktı üretimi | `ascii_cikti_boyutlari_tamdir`, `ascii_cikti_deterministiktir`, `ascii_cikti_kutu_sinirlarini_ayirt_edilebilir_kilar`, `ascii_cikti_en_buyuk_kutuya_en_yogun_karakteri_verir`, `cli_treemap_renksiz_ascii_uretir` |
| Atomik yazma | `anlik_goruntu_yarim_kalmis_gecici_dosya_birakilmaz` |
| **Veri silinmez** | `hicbir_komut_dosya_silmez_ya_da_degistirmez` |

### "Hiçbir komut dosya silmez ya da değiştirmez" kuralının kanıtı

`tests/entegrasyon.rs` → `hicbir_komut_dosya_silmez_ya_da_degistirmez`:

1. Demo fixture'inin tüm dosyaları için `(göreli yol, boyut, mtime, içerik baytları)`
   parmak izi alınır.
2. Yedi alt komutun **tamamı** gerçek `disktree.exe` ikilisiyle çalıştırılır
   (`scan`, `tree`, `treemap`, `report`, `bigfiles`, `timeline`, `heatmap`).
   Çıktı dosyaları taranan ağacın **dışına** yazılır, aksi hâlde "dosya sayısı
   değişmedi" iddiası kendi kendini bozardı.
3. Parfak izi yeniden alınır ve **her dosya için** yol kümesi, boyut, mtime ve
   içerik baytları karşılaştırılır. Dosya silinmiş, küçülmüş, yeniden yazılmış
   (mtime değişirdiği için yakalanır) veya taranan ağaca yeni dosya eklenmişse
   test çöker.
4. Ayrıca fixture içerikleri ayrıca bayt bayt doğrulanır.

Bunun yanında kaynak kod düzeyinde de yazma yolu yoktur: `src/` altındaki hiçbir
dosya `File::create`, `fs::write`, `fs::remove_*`, `OpenOptions` veya
`set_permissions` çağırmaz. Tek yazma noktası, kullanıcının `--cikti` ile açıkça
verdiği JSON dosyasıdır (`src/rapor.rs`).

### Kalite kapısı

```console
$ cargo build --release
    Finished `release` profile [optimized] target(s) in 27.96s

$ cargo test
test result: ok. 118 passed; 0 failed; ... (yukarıda tam çıktı)

$ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s

$ cargo fmt --check
(çıktı yok — biçim temiz)
```

Dördü de yeşildir; `cargo clippy` ve `cargo fmt --check` çıkış kodu `0`'dır.
Ayrıca `cargo build --release` ikilisi 1 296 910 bayttır ve çalışma zamanında
hiçbir DLL dışında tek dosyadır.

---

## Proje Yapısı

```
06-disktree/
├── Cargo.toml                 # bağımlılıklar ve [profile.release]
├── Cargo.lock                 # üretilir, commit edilir
├── LICENSE.txt                # MIT tam metni
├── README.md
├── .gitignore                 # target/, .env, *.log, demo/
├── scripts/
│   └── demo-olustur.ps1       # README örneklerinin demo ağacını üretir
├── src/
│   ├── lib.rs                 # çekirdek mantık + crate lint'leri
│   ├── main.rs                # CLI kabuğu (clap)
│   ├── hata.rs                # Hata enum'u, Display + Error
│   ├── model.rs               # YolHavuzu + düz Kayit dizisi (Agac)
│   ├── tarama.rs              # read_dir yürüyüşü, DonguKoruma
│   ├── treemap.rs             # squarified yerleşim + ASCII ızgara
│   ├── zaman.rs               # takvim dönüşümü, aylık kume, fark
│   ├── isiharitasi.rs         # degisim × derinlik skoru
│   ├── buyuk.rs               # büyük/eski/gizli liste, arşiv dışlama
│   ├── ayirma.rs              # kayıp + küme tahmini
│   └── rapor.rs               # serde şemaları, atomik JSON okuma/yazma
└── tests/
    └── entegrasyon.rs         # 26 uçtan uca test
```

Satır sayıları (Rust kaynakları):

| Dosya | Satır |
|---|---:|
| `src/ayirma.rs` | 273 |
| `src/buyuk.rs` | 342 |
| `src/hata.rs` | 225 |
| `src/isiharitasi.rs` | 254 |
| `src/lib.rs` | 65 |
| `src/main.rs` | 665 |
| `src/model.rs` | 749 |
| `src/rapor.rs` | 552 |
| `src/tarama.rs` | 838 |
| `src/treemap.rs` | 590 |
| `src/zaman.rs` | 416 |
| `tests/entegrasyon.rs` | 625 |
| **Toplam** | **5 594** |

Modül bağımlılık yönü tek yönlüdür: `hata` ← `model` ← {`tarama`, `treemap`,
`zaman`} ← {`isiharitasi`, `buyuk`, `ayirma`, `rapor`} ← `main`.

---

## Yapılandırma

DiskTree yapılandırma **dosyası kullanmaz**; tüm ayarlar komut satırı bayraklarıdır.
Böylece USB'den çalıştırılan ikili, yanında ayar dosyası aramaz ve salt okunur
ortamda da aynı davranışı gösterir. (`MANIFEST.md` kartı "yapılandırma program
dizininde" öngörür; bu MVP'de bayraklara indirgenmiştir ve `## Bilinen
Sınırlamalar` bölümünde yazılıdır.)

### Ortak tarama bayrakları

`scan`, `tree`, `treemap`, `bigfiles` ve `report` alt komutlarında ortaktır.

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--en-derinlik <N>` | `64` | Kökten (0) aşağı en fazla bu derinliğe inilir. Aşılan dizinler `derinlik_asim` sayacına girer. |
| `--gizlileri-haric-et` | kapalı | Gizli öznitelikli girdiler ağaca **eklenmez** (Windows `FILE_ATTRIBUTE_HIDDEN`; diğer platformlarda nokta ile başlayan adlar). |
| `--sistemleri-haric-et` | kapalı | Sistem öznitelikli girdiler ağaca eklenmez. |
| `--sembolik-bag` | kapalı | Sembolik bağların hedefi de gezilir. Kapalıyken bağ dizinlere inilmez. Her iki kipte de `DonguKoruma` kanonik yol kümesiyle sonsuz döngüyü keser. |
| `--haric <KALIP>` | yok | Verilen kalıbı içeren yollar atlanır. Eşleşme **alt dizi** aramasıdır, büyük/küçük harf duyarsızdır ve baştaki `*` yok sayılır. Birden çok kez verilebilir. Örnek: `--haric node_modules --haric "*.zip"`. |
| `--maks-dugum <N>` | yok | Bu düğüm sayısına ulaşılınca tarama kısmi biter; `kisaltildi=true` ve `tamamlandi=false` raporlanır. |

### `scan`

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--cikti <DOSYA>` | yok | Anlık görüntü JSON'unu bu dosyaya **atomik** yazar (geçici dosya + `rename`). Yazılmazsa yalnızca ekrana özet basılır. |
| `--sessiz` | kapalı | Hiçbir şeyi ekrana basmaz; yalnızca çıkış kodu döner. |

### `tree`

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--derinlik <N>` | `3` | Metin ağacında görüntülenecek azami derinlik. |
| `--ust-puan <N>` | `20` | Her düzeyde gösterilecek azami çocuk sayısı; kalanı "... N cocuk daha" ile özetlenir. |

### `treemap`

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--genislik <N>` | `100` | ASCII ızgara genişliği (terminal karakteri). |
| `--yukseklik <N>` | `30` | ASCII ızgara yüksekliği (terminal satırı). |
| `--en-cok <N>` | `12` | Kutuya dönüştürülecek en büyük çocuk sayısı. |
| `--alt <YOL>` | kök | Treemap'in verileceği alt dizin. Bulunamazsa `Hata::YolBulunamadi` döner ve çıkış kodu `1` olur. |
| `--cikti <DOSYA>` | yok | Kutuların JSON'unu yazar. |

### `timeline` / `heatmap`

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--en-cok <N>` | `25` | Ekranda gösterilecek azami satır. |
| `--aylik` (yalnız `timeline`) | kapalı | Aylık kümülatif büyüme eğrisini de basar. |
| `--cikti <DOSYA>` | yok | Sonuçları JSON olarak yazar. |

### `bigfiles`

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--esik-bayt <BAYT>` | `524288000` (500 MiB) | Boyut eşiği altındaki dosyalar listelenmez. |
| `--yas-gun <GUN>` | `365` | Son değişikliğinden daha yeni dosyalar listelenmez. |
| `--en-cok <N>` | `25` | Azami satır sayısı. |
| `--arsivleri-dahil-et` | kapalı | Varsayılanda `zip, 7z, rar, tar, gz, bz2, xz, zst, iso, cab` uzantılı dosyalar listeden çıkarılır. |
| `--gizlileri-liste-haric` | kapalı | Gizli/sistem dosyaları **listeden** çıkarır. (Taramadan çıkarmak için ortak bayrak olan `--gizlileri-haric-et` kullanılır; iki işlev farklı olduğu için ayrı adlandırılmıştır.) |

### `report`

| Bayrak | Varsayılan | Etki |
|---|---|---|
| `--kapasite <BAYT>` | `0` | Birim kapasitesi. Ayırma denetiminin girdisidir. |
| `--bos <BAYT>` | `0` | Dosya sisteminin bildirdiği boş alan. |
| `--yedeklenmis <BAYT>` | `0` | NTFS yedeklenmiş küre (backup stream) miktarı. |
| `--kume <BAYT>` | `4096` | Dosya sistemi küme boyutu. `0` verilirse `Hata::GirdiGecersiz` döner. |
| `--cikti <DOSYA>` | **zorunlu** | Rapor JSON'unun yazılacağı dosya. |

### Anlık görüntü dosya biçimi

`--cikti` ile yazılan JSON'un imzası `disktree-kayma/1` ve şema sürümü `1`'dir.
Okurken ikisi de doğrulanır; uyuşmazlıkta dosya **sessizce kullanılmaz**, hata
verilir:

```json
{
  "bicim": "disktree-kayma/1",
  "surum": 1,
  "kok": "C:\\Users\\xXx\\Desktop\\Projeler\\projects\\06-disktree\\demo",
  "zaman": 1790679645,
  "dugumler": [
    {
      "ad": "C:\\Users\\xXx\\Desktop\\Projeler\\projects\\06-disktree\\demo\\arsivler",
      "tur": "dizin",
      "boyt": 0,
      "alt_toplam": 0,
      "derinlik": 1,
      "degistirme": 1790679644,
      "gizli": false,
      "sistem": false
    }
  ]
}
```

### Ortam değişkenleri

DiskTree hiçbir ortam değişkenini okumaz. `NO_COLOR` / `CLICOLOR_FORCE` yalnızca
**renk** üretimi için anlamlıdır ve DiskTree hiç renk üretmez; terminal çıktısı
her zaman renksizdir.

---

## Bilinen Sınırlamalar

Bu bölüm kasıtlı olarak dürüsttür. `MANIFEST.md` kartındaki ertelenen liste ve bu
MVP'de ortaya çıkan sınırlar burada toplanmıştır.

### MFT okunmadığından doğan sınırlar

1. **Kayıp + küme değeri ölçüm değil, tahmindir.** Denklemin ölçülebilir iki
   terimi (`yedeklenmis`, `bos`) NTFS denetim bilgisinden okunur; MFT okunmadığı
   için bu değerler **kullanıcıdan `--bos` / `--yedeklenmis` ile alınır**. Sonuç
   bu yüzden "ölçülen gerçek kayıp küme miktarı" değildir. Negatif çıkarsa bu
   veri kaybı değil, modelin eksik olduğunun göstergesidir ve araç bunu açıkça
   `uyari:` satırıyla bildirir.
2. **Birim düzeyinde hiçbir sayı okunmaz.** `System Volume Information` altındaki
   yedekleme küre sayacı, `$MFT` boyutu, `$Bitmap` serbest küre sayacı ve
   muf/start alanı okunmaz. `--kapasite` ve `--bos` kullanıcıdan gelir.
3. **NTFS sıkıştırılmış, sparse ve şifreli dosyalarda boyut yanıltıcıdır.**
   `std::fs::Metadata::len()` **mantıksal** boyutu verir; diskte ayrılmış alanı
   vermez. Birim doluluğu ile toplam mantıksal boyut bu yüzden ayrışabilir.
4. **Sıfır boyutlu dosyalar treemap'te görünmez.** Sıfır alanlı kutular sonsuz
   döngüye yol açacağı için yerleşime alınmaz; ağaçta sayılırlar ama haritada
   çizilmezler.
5. **NTFS alternatif veri akışları (ADS) taranmaz.**

### 32-bit düğüm kimliği (rapordan sapma)

Rapor `b07`'de 48-bit MFT referansı öngörür. DiskTree MFT okumadığı için kimlik,
`Vec<Kayit>` dizisindeki **32-bit indekstir** ve tavan 2^32 düğümdür. Ölçülen
kayıt boyutu **72 bayttır** (`YolKimlik` 8, `DugumId` 4, `Tur` 1 bayt) ve bu,
raporun "düğüm başına 64 bayt" varsayımıyla uyumludur: 2 000 000 kayıt
≈ 137 MiB, 4 000 000 kayıt ≈ 274 MiB (raporun `≤ 320 MB` tavanına sığar). Daha
yüksek hacimler için `--maks-dugum` ile tarama bilinçli olarak kısaltılabilir; bu
durumda rapor `kisaltildi=true` ile işaretlenir.

### Bellek bütçesi ölçülmedi

Rapor `≤ 180 MB` bütçeyi **tahmin** olarak verir. Bu MVP'de bellek bütçesi
**ölçülmemiştir**; yalnızca model yapısı (havuz + düz dizi, düğüm başına `Vec`
yok) raporun bütçe varsayımıyla uyumludur. Gerçek tepe RSS ölçümü ayrı bir
iştir ve yapılmamıştır. Demo ağacında (22 düğüm, göreli yol) model `2.7 KiB`
tutmuştur.

### Zaman çizelgesi iki anlık görüntü ister

`timeline` ve `heatmap` **iki** anlık görüntüyü karşılaştırır. Karşılaştırma
**metinsel yol eşitliğine** dayanır: iki taramada kök yol **aynı biçimde**
yazılmış olmalıdır. Birincisi `demo`, ikincisi mutlak yol ise tüm yollar "yeni"
ve "silinmiş" görünür. Bu bir hata değil, modelin sınırıdır; anlık görüntüdeki
`kok` alanı her iki dosyada da kontrol edilmelidir.

### Sıralama ve tahminler

- **Gizli dosya tespiti istatistiksel bir gözlemdir**, kötücül yazılım hükmü
  değildir. Araç bir antivirüs, casus yazılım tarayıcısı veya savunma aracı olarak
  konumlanmaz ve öyle sunulmamalıdır.
- **Yapılandırma dosyası yoktur.** `MANIFEST.md` kartı "yapılandırma program
  dizininde `config/`" öngörür; bu MVP'de tüm ayarlar bayraktadır. Kalıcı ayar
  ihtiyacı olan kullanıcı için doğal sonraki adım bir profil dosyasıdır.
- **Windows dışı platformlarda gizli/sistem özniteliği zayıftır.** Standart
  kütüphane POSIX sistemlerinde gizli dosya bilgisi vermediği için nokta ile
  başlayan adlar gizli sayılır ve `sistem` her zaman `false` olur.
- **Sembolik bağ izlemesi varsayılan kapalıdır.** Açıldığında döngü koruması
  devreye girer ama aynı veriye birden çok yoldan ulaşmak (sert bağlar) sayımları
  yine de şişirebilir; bağlar ağaçta ayrı bir işaret taşır (`bag` sayacı).

### ertelenmiş özellikler (`MANIFEST.md`)

- ext4 ve APFS ham okuyucuları
- Ayırma denetiminin **ölçüm** hâli (NTFS denetim bilgisinden okuma)
- HTML/PDF dışa aktarma
- USN/inotify/FSEvents tabanlı canlı değişim izleme
- Sıkıştırılmış önbellek dosyası
- Çok iş parçacılı okuma (rapor 8 iş parçacığı tavanı öneriyor; bu MVP tek iş
  parçacığıdır)

### `#[allow]` kullanımı

Kod tabanında yalnızca **iki** `allow` vardır ve ikisi de gerekçelidir:

1. `src/lib.rs` ve `src/main.rs`: `#![cfg_attr(test, allow(clippy::unwrap_used,
   clippy::expect_used))]`. Üretim kodunda `unwrap`/`expect` yasağı yerinde
   kalır; bu `allow` yalnızca `#[cfg(test)]` modüllerinde, test fixture'ı kurarken
   kullanılan `expect` çağrılarına izin verir (`WORKER_CONTRACT.md` § 4.2:
   "yalnızca testlerde ve burada da gerekçeyle kullanılabilir").
2. `src/treemap.rs`: `#[allow(clippy::too_many_arguments)]` — `satir_yerlestir`
   iç yardımcısı; parametreler tek bir çağrıda sabittir ve bir araya getirilmesi
   yerleşim algoritmasının okunabilirliğini bozardı. Dışa açık API'de bu
   gerekçe daha da kısıtlıdır: `Agac::ekle` dokuz konumsal argüman (üçü `bool`)
   alıyordu ve **iki `bool` özniteliğin yer değiştirmesi ihtimali nedeniyle**
   `DuzumGirdi` yapısına dönüştürüldü.

### Test ortamına bağlı sınırlar

- **Gerçek ACL tabanlı izin reddi** üretilemez: NTFS izinlerini değiştirmek
  `icacls` gibi harici bir araç gerektirir ve bu, taşınabilirlik ilkesine aykırıdır.
  Bunun yerine (a) bir dosyayı dizin kökü olarak vermek — işletim sisteminin
  gerçek `ENOTDIR` hatasını üretir — ve (b) döngü korumasının sayacı test edilir.
  Windows'ta gizli öznitelik testi, `attrib +H` komutuyla (her Windows
  kurulumunda bulunan yerleşik araç) dosya niteliğini değiştirir.
- **Sembolik bağ oluşturma** Windows'ta Geliştirici Modu veya yönetici yetkisi
  gerektirir. Bu nedenle döngü korumasının uçtan uca kanıtı, `DonguKoruma::tohumla`
  ile belirli bir dizine girişi bilinçli olarak yaslayan **gerçek dosya sistemi**
  testiyle yapılır; sembolik bağ oluşturulabilirse ek olarak sembolik bağ
  izleme testi de koşar. Her iki yol da `DonguKoruma::dongu` sayacının beklenen
  değerini doğrular.
- Zaman ölçümleri (`sure_sn`) yalnızca çıktı içindir; hiçbir test kararına girmez.

---

## Gelecek Geliştirmeler

1. **Profil dosyası** (`config/profile.json`): kalıcı ayarlar ve dışlama listeleri.
   Raporun "yapılandırma program dizininde" önerisinin karşılığı.
2. **Ayırma denetiminin ölçüm hâli:** birim kapasitesi ve boş küme sayısını
   okuyan, yalnızca Windows'ta ve yalnızca kullanıcı açıkça isteyip yönetici
   yetkisi verirse çalışan isteğe bağlı katman. Varsayılan kapalı kalır.
3. **Sıkıştırılmış önbellek** — `flate2` (`miniz_oxide`) yalnızca 04 ve 29'da
   serbest olduğu için bu projede kullanılamaz; alternatif olarak kendi
   sözlük sıkıştırmamız gerekir. Şu an düz JSON yazılır.
4. **İlerleme göstergesi** ve `Ctrl-C` ile temiz kesme (raporun R3 risk azaltması).
5. **Statik HTML/SVG treemap dışa aktarımı** — karar D-010'un kabul ettiği tek
   görsel çıktı biçimi statik HTML'dir; pencere/WebView yasaktır.
6. **Yol maskeleme seçarıcı** (raporun R10 riski): rapor paylaşılırken kullanıcı
   adı ve kişisel klasör adlarının değiştirilmesi.
7. **Çok iş parçacılı tarama** (raporun 8 iş parçacığı tavanı).
8. **USN Journal tabanlı kısmi tarama** — ikinci taramada yalnızca farkı okumak.

---

## Troubleshooting

### 1) `disktree: hata: tarama kökü okunamadi: C:\... (The system cannot find the path specified.)`

**Belirti:** Çıkış kodu `1`, `tarama kökü okunamadi` mesajı.
**Neden:** Verilen yol yok, ya da yol bir dosyadır (`read_dir` dosyada `ENOTDIR`
verir), ya da yol Türkçe karakter içeriyorsa kabuk tarafından bozulmuştur.
**Çözüm:** Yolun var olduğunu doğrulayın; göreli yol yerine mutlak yol kullanın.
Dizin olduğundan emin değilseniz `tree <yol>` ile deneyin. PowerShell'de
`Test-Path -PathType Container .\yol` kontrolü işe yarar.

### 2) Timeline çıktısında her yol "yeni" ve "silinmis" görünüyor

**Belirti:** `1. yeni ... 2. silinmis ...` — gerçekte değişen bir şey yok.
**Neden:** İki anlık görüntü **farklı kök yazımıyla** alınmıştır (ör. biri
`demo`, diğeri mutlak yol). Karşılaştırma metinsel yol eşitliğine dayanır.
**Çözüm:** İki taramayı da aynı yazımla alın:

```console
$ disktree scan C:\...\demo --cikti onceki.json
$ disktree scan C:\...\demo --cikti sonraki.json
$ disktree timeline onceki.json sonraki.json
```

İçindeki `kok` alanlarını karşılaştırarak doğrulayabilirsiniz.

### 3) Ayırma denetiminde `kayip` negatif ve `uyari:` satırı var

**Belirti:** `kayip: -3112 bayt (tahmin, olcum degildir)` ve bir `uyari:` satırı.
**Neden:** Verilen `--kapasite` / `--bos` değerleri gerçek birimi yansıtmıyor ya da
taranan ağaç birimden büyük (ör. bir ağ sürücüsü veya bağlı bir disk taranmış).
Bu veri kaybı **değildir**.
**Çözüm:** `--kapasite` ve `--bos` değerlerini işletim sisteminin kendi
bildiriminden kopyalayın. Hatırlatın: MFT okunmadığı için bu iki terim zaten
kullanıcıdan gelir.

### 4) Treemap çıktısı boş veya kutular görünmüyor

**Belirti:** Izgara boş, veya "4 kutu" yerine "0 kutu" yazıyor.
**Neden:** (a) `--alt` ile verilen yol bulunamadı (bu durumda araç açık hata
verir, çıktı üretmez); (b) taranan alt dizindeki tüm dosyalar 0 bayt; (c) seçilen
alt dizin çok küçük (ör. 1×1 ızgara).
**Çözüm:** `--en-cok` değerini artırın, `--genislik`/`--yukseklik` değerlerini
büyütün ve taranan yolun altında gerçekten dosya olduğunu `tree` ile doğrulayın.

### 5) `scan` çok yavaş veya `erisim_reddi` çok yüksek

**Belirti:** Tarama yavaş; özet satırında `erisim_reddi=412`.
**Neden:** Yönetici değilsiniz ve `System Volume Information` gibi korumalı
yollara erişilemiyor. Her biri sayılır ve `scan` çıktısında `kisitli:` satırı
olarak listelenir; tarama yine de tamamlanır.
**Çözüm:** Kapsamı bilerek daraltın: `--haric` ile korumalı yolları dışlayın veya
`--en-derinlik`/`--maks-dugum` ile taramayı sınırlandırın. Yönetici yetkisi
istemek **kasıtlı olarak desteklenmez** (bkz. "Rapor sapması").

### 6) `linker 'link.exe' not found` derleme hatası

**Belirti:** `cargo build` başarısız, linker bulunamadı.
**Neden:** Rust araç zinciri (`link.exe`) PATH'te değil.
**Çözüm:** Bu ortamda her `cargo` çağrısından önce PATH'i ayarlayın:

```powershell
$env:PATH = "%USERPROFILE%\.cargo\bin;%USERPROFILE%\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;" + $env:PATH
```

### 7) Antivirüs uyarısı

**Belirti:** Windows Defender veya başka bir antivirüs `disktree.exe` dosyasını
uyarıyla işaretliyor.
**Neden:** Disk düzeyinde okuma yapan, yeni derlenmiş küçük bir ikili, davranış
tabanlı motorlarda "masaüstü uygulaması" sınıfına girmez ve şüpheli sayılabilir.
**Çözüm:** Antivirüs istisnası eklemeyin. Kaynak kod açıktır; `cargo build
--release` komutuyla yerelde kendiniz derleyip davranışı denetleyebilirsiniz. Bu
durumda ne olacağı README'nin bu bölümünde önceden yazılmıştır.

---

## Atıflar

Bu bölüm MIT lisansının atıf yükümlülüğünün ötesinde, bu orkestrasyonun
zorunlu kıldığı kaynak listesidir.

### Rapor dosyası (yerel yol, URL değildir)

- **DiskAğacı fikir raporu (06/30)** — `%USERPROFILE%\Desktop\Fikirler\06-disk-agaci-analizor.html`.
  Ürün tanımı, kullanım senaryoları, kabul kriterleri, bellek bütçesi ve risk
  analizi bu belgeden alınmıştır. Bölüm numaraları (`b01`, `b03`, `b05`, `b07`,
  `b08`, `b09`, `b10`, `b16`) atıfta geçen satırlarla eşleşir.
- **`MANIFEST.md` kart 06** — `%USERPROFILE%\Desktop\Projeler\MANIFEST.md`.
  Uygulama stack'i, sapma gerekçesi ve ertelenen özellik listesi bu karttan
  alınmıştır.
- **`WORKER_CONTRACT.md`** — `%USERPROFILE%\Desktop\Projeler\WORKER_CONTRACT.md`.
  Bağımlılık politikası, kalite kapısı ve teslim kuralları bu belgeden alınmıştır.

### Uygulanan algoritma ve spesifikasyon

- **Bruls, Huizing, van Wijk — "Squarified Treemaps"** (2000), Advances in
  Information and Human-Computer Interaction. Yerleşim kuralı bu makaledeki
  "worst aspect ratio" ölçütünden türetilmiştir.
  <https://www.win.tue.nl/~vanwijk/stm.pdf>
- **Howard Hinnant — `chrono`-Compatible Low-Level Date Algorithms**
  (`days_from_civil` / `civil_from_days`). Takvim dönüşümü bu algoritmayla
  uygulanmıştır; `chrono`/`time` crate'leri yasak olduğu için elle yazılmıştır.
  <https://howardhinnant.github.io/date_algorithms.html>

### Referans alınan ve modellenen açık kaynak projeler

- **GNU `du`** — özyinelemeli disk kullanımı hesabının klasik referansı.
  <https://www.gnu.org/software/coreutils/manual/html_node/du-invocation.html>
- **NCdu** — ncurses tabanlı terminal disk kullanımı gezgini; terminal çıktı
  düzeninin referansı olarak incelendi.
  <https://dev.yorhel.nl/ncdu>
- **WizTree** — NTFS MFT'sini doğrudan okuyan ticari disk analizörü; MFT okuma
  yaklaşımının ve "deki katmanı" fikrinin kaynağı.
  <https://diskanalyzer.com/>
- **WinDirStat** — GPL lisanslı, Gezgin kabuğuyla çalışan disk kullanımı
  görselleştiricisi. <https://windirstat.net/>
- **TreeSize** — Windows için ticari disk analizörü. <https://www.jam-software.com/treesize>

### NTFS ve işletim sistemi kaynakları (raporun atıfları)

- **Microsoft Learn — NTFS** — MFT kavramının birincil açıklaması.
  <https://learn.microsoft.com/windows-server/storage/file-server/ntfs>
- **Microsoft Learn — USN Journal** — değişiklik günlüğünün yapısı (ileride
  planlanan kısmi tarama için). <https://learn.microsoft.com/windows/win32/fileio/usn>
- **Microsoft Learn — DeviceIoControl** — ham disk okuma yüzeyi.
  <https://learn.microsoft.com/windows/win32/api/winioctl/nf-winioctl-deviceiocontrol>
- **Microsoft Learn — Dosya öznitelik sabitleri** — `SYSTEM`, `HIDDEN`,
  `REPARSE_POINT` bitlerinin anlamı.
  <https://learn.microsoft.com/windows/win32/fileio/file-attribute-constants>

### Kullanılan Rust crate'leri ve Rust kaynakları

- **Rust standart kütüphane belgeleri** — `std::fs`, `std::io`, `std::time`,
  `std::os::windows::fs::MetadataExt`.
  <https://doc.rust-lang.org/std/>
- **Rust edition rehberi (2021)** — <https://doc.rust-lang.org/edition-guide/edition-2021/>
- **`serde`** — türetilmiş veri yapıları. <https://serde.rs/> · <https://docs.rs/serde/>
- **`serde_json`** — JSON okuma/yazma. <https://github.com/serde-rs/json>
- **`clap`** — CLI argüman ayrıştırma. <https://docs.rs/clap/>
- **Semantic Versioning** — sürüm numaralandırma ilkeleri. <https://semver.org/>

### Hiç kullanılmayan, yasaklanan teknolojiler

Aşağıdakilerin **hiçbiri** projede yer almaz; bu, bilinçli bir kapsam
kararıdır: `egui`, `winit`, `gtk`, `wry`, WebView, `treemap`, `walkdir`,
`notify`, `tokio`, `reqwest`, `chrono`/`time`, `flate2`, `rusqlite`, `gix`.
Gerekçeler ve yerine ne konduğu `## Rapor sapması` ile `## Bilinen Sınırlamalar`
bölümlerindedir.

---

## Lisans

**MIT.** Kaynak kod lisansı MIT'tir; tam metin `LICENSE.txt` dosyasındadır.

```
Copyright (c) 2026 DiskTree contributors
```

Telif hakkı bildirimi: bu depodaki kaynak kod MIT lisansıyla dağıtılmaktadır.
Rapor (06/30) da MIT önermektedir; lisans uyumsuzluğu doğmaz.

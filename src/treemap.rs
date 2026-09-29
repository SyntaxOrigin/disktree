//! Squarified treemap yerleşimi ve renksiz terminal (ASCII) çıktısı.
//!
//! Algoritma Bruls, Huizing ve van Wijk'in *"Squarified Treemaps"* makalesindeki
//! yerleşim kuralını birebir uygular: değerler büyükten küçüğe sıralanır, her
//! adımda mevcut şeridin "en kötü en-boy oranı" iyileştiği sürece yeni kutu
//! şeride eklenir, aksi hâlde şerit yerleştirilip yeni şerit açılır.
//!
//! `treemap` / `egui` crate'i bağımlılık politikasıyla yasak olduğu için
//! yerleşim ve hücre çizimi elden yazılmıştır. Grafik arayüz de yasak olduğu
//! için tek çıktı renksiz ASCII ızgaradır.
//!
//! Yerleşimin doğrulanan değişmezleri:
//!
//! * Her kutunun alanı, değerinin toplam alandaki oranıyla tam olarak aynıdır
//!   (`genislik * yukseklik == deger / toplam_deger * toplam_alan`).
//! * Kutular toplam dikdörtgenin içinde kalır ve birbirleriyle kesişmez.
//! * `deger == 0` olan girdiler yerleşime alınmaz (sonsuz döngü kaynağı olurdu).

use serde::{Deserialize, Serialize};

/// Bir girdi: etiket + gösterilecek değer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Girdi {
    /// Kutu içinde gösterilecek ad.
    pub etiket: String,
    /// Alanı belirleyen pozitif değer (bayt).
    pub deger: u64,
}

impl Girdi {
    /// Verilen etiket ve değerle yeni bir girdi oluşturur.
    pub fn yeni(etiket: impl Into<String>, deger: u64) -> Self {
        Self {
            etiket: etiket.into(),
            deger,
        }
    }
}

/// Yerleştirilmiş tek bir treemap kutusu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kutu {
    /// Girdi etiketi.
    pub etiket: String,
    /// Girdi değeri (bayt).
    pub deger: u64,
    /// Sol kenar (mantıksal birim).
    pub x: f64,
    /// Üst kenar (mantıksal birim).
    pub y: f64,
    /// Genişlik (mantıksal birim).
    pub genislik: f64,
    /// Yükseklik (mantıksal birim).
    pub yukseklik: f64,
}

impl Kutu {
    /// Kutunun alanı.
    #[must_use]
    pub fn alan(&self) -> f64 {
        self.genislik * self.yukseklik
    }

    /// Kutunun merkezi.
    #[must_use]
    pub fn merkez(&self) -> (f64, f64) {
        (self.x + self.genislik / 2.0, self.y + self.yukseklik / 2.0)
    }
}

/// Alanı geçersiz (sıfır veya negatif) bir dikdörtgen içinde yerleşim yapar.
///
/// Değeri `0` olan girdiler atlanır. Tüm değerler `0` ise boş vektör döner.
///
/// Uygulama, değerleri baştan "alan birimine" çevirip (`deger * olcek`)
/// algoritmayı ölçekli değerlerle çalıştırır. Böylece her kutunun alanı
/// girdi değerinin toplam alandaki oranıyla birebir aynı olur ve sonradan
/// düzeltme geçişi gerekmez.
#[must_use]
pub fn yerlestir(girdiler: &[Girdi], x: f64, y: f64, genislik: f64, yukseklik: f64) -> Vec<Kutu> {
    // NaN ve sonsuz değerler de reddedilir: kısmi sıralı tipte `<=` yerine
    // açık `is_finite` denetimi kullanmak, hem NaN'ı hem de sonsuzu elemeyi
    // sağlar.
    if !genislik.is_finite() || genislik <= 0.0 || !yukseklik.is_finite() || yukseklik <= 0.0 {
        return Vec::new();
    }
    let mut sirali: Vec<&Girdi> = girdiler.iter().filter(|g| g.deger > 0).collect();
    if sirali.is_empty() {
        return Vec::new();
    }
    // Değer eşitliğinde etiket sırası belirleyici olsun: çıktı deterministiktir.
    sirali.sort_by(|a, b| b.deger.cmp(&a.deger).then_with(|| a.etiket.cmp(&b.etiket)));
    let degerler: Vec<u64> = sirali.iter().map(|g| g.deger).collect();
    let etiketler: Vec<&str> = sirali.iter().map(|g| g.etiket.as_str()).collect();
    let toplam: f64 = degerler.iter().map(|&d| d as f64).sum();
    let olcek = (genislik * yukseklik) / toplam;
    let alan_birimleri: Vec<f64> = degerler.iter().map(|&d| d as f64 * olcek).collect();

    let mut sonuc: Vec<Kutu> = Vec::with_capacity(sirali.len());
    let mut kalan = Alan {
        x,
        y,
        genislik,
        yukseklik,
    };
    let mut i = 0usize;

    while i < alan_birimleri.len() {
        // Kalan alan çok küçüldüyse artan kutular sıfır alanla konumlandırılır;
        // bu, sonsuz döngüyü ve sıfıra bölme hatasını engeller.
        if kalan.kisa_kenar() < 1e-9 {
            for j in i..alan_birimleri.len() {
                sonuc.push(Kutu {
                    etiket: etiketler[j].to_string(),
                    deger: degerler[j],
                    x: kalan.x,
                    y: kalan.y,
                    genislik: 0.0,
                    yukseklik: 0.0,
                });
            }
            break;
        }
        let kisa = kalan.kisa_kenar();
        let mut satir: Vec<usize> = vec![i];
        let mut satir_toplam = alan_birimleri[i];
        let mut satir_min = alan_birimleri[i];
        let mut satir_maks = satir_min;
        let mut mevcut_kotuluk = kotuluk(satir_toplam, 1.0, satir_min, satir_maks, kisa);

        let mut j = i + 1;
        while j < alan_birimleri.len() {
            let d = alan_birimleri[j];
            let yeni_kotuluk = kotuluk(
                satir_toplam + d,
                (satir.len() + 1) as f64,
                satir_min.min(d),
                satir_maks.max(d),
                kisa,
            );
            if yeni_kotuluk <= mevcut_kotuluk {
                satir.push(j);
                satir_toplam += d;
                satir_min = satir_min.min(d);
                satir_maks = satir_maks.max(d);
                mevcut_kotuluk = yeni_kotuluk;
                j += 1;
            } else {
                break;
            }
        }
        kalan = satir_yerlestir(
            &kalan,
            &satir,
            &alan_birimleri,
            &etiketler,
            &degerler,
            satir_toplam,
            &mut sonuc,
        );
        i = j;
    }
    sonuc
}

/// Bir şeridi kalan alana yerleştirir ve kalan alanı döndürür.
///
/// `dikey` durumda şerit, kalan dikdörtgenin solunda dikey bir banttır: her
/// kutu bandın tam genişliğinde, yüksekliği değer oranı kadardır. `yatay`
/// durumda bunun 90 derecelik karşılığı uygulanır.
#[allow(clippy::too_many_arguments)]
fn satir_yerlestir(
    kalan: &Alan,
    satir: &[usize],
    agirliklar: &[f64],
    etiketler: &[&str],
    degerler: &[u64],
    satir_toplam: f64,
    sonuc: &mut Vec<Kutu>,
) -> Alan {
    let dikey = kalan.genislik >= kalan.yukseklik;
    let kalinlik = (satir_toplam / kalan.kisa_kenar()).clamp(
        0.0,
        if dikey {
            kalan.genislik
        } else {
            kalan.yukseklik
        },
    );
    let mut ofset = if dikey { kalan.y } else { kalan.x };
    for &idx in satir {
        let oran = agirliklar[idx] / satir_toplam;
        let kutu_olcu = if dikey {
            (kalinlik, kalan.yukseklik * oran)
        } else {
            (kalan.genislik * oran, kalinlik)
        };
        let (kx, ky) = if dikey {
            (kalan.x, ofset)
        } else {
            (ofset, kalan.y)
        };
        sonuc.push(Kutu {
            etiket: etiketler[idx].to_string(),
            deger: degerler[idx],
            x: kx,
            y: ky,
            genislik: kutu_olcu.0,
            yukseklik: kutu_olcu.1,
        });
        ofset += kutu_olcu.1.max(0.0);
    }
    if dikey {
        Alan {
            x: kalan.x + kalinlik,
            y: kalan.y,
            genislik: (kalan.genislik - kalinlik).max(0.0),
            yukseklik: kalan.yukseklik,
        }
    } else {
        Alan {
            x: kalan.x,
            y: kalan.y + kalinlik,
            genislik: kalan.genislik,
            yukseklik: (kalan.yukseklik - kalinlik).max(0.0),
        }
    }
}

/// Bir şeritteki kutuların en kötü en-boy oranı.
///
/// `kisa_kenar` dikdörtgenin daha kısa olan kenarıdır; şerit bu kenar boyunca
/// uzanır. En kötü oran, çapraz (köşegen benzeri) kutularda büyür.
fn kotuluk(toplam: f64, adet: f64, en_kucuk: f64, en_buyuk: f64, kisa_kenar: f64) -> f64 {
    if toplam <= 0.0 || adet <= 0.0 || kisa_kenar <= 0.0 {
        return f64::INFINITY;
    }
    let t2 = toplam * toplam;
    let k2 = kisa_kenar * kisa_kenar;
    (t2 / (k2 * en_kucuk)).max(k2 * en_buyuk / t2)
}

/// Kalan alanın kısa kenarı.
#[derive(Debug, Clone, Copy)]
struct Alan {
    x: f64,
    y: f64,
    genislik: f64,
    yukseklik: f64,
}

impl Alan {
    fn kisa_kenar(&self) -> f64 {
        self.genislik.min(self.yukseklik)
    }
}

/// Terminal yoğunluk rampası (ilk karakter boşluk, son karakter `@`).
const RAMP: &[u8] = b" .:-=+*#%@";

/// Kutuları renksiz bir ASCII ızgarasına çizer.
///
/// Her hücre, bulunduğu kutunun **alan sıralamasına** göre `RAMP` dizisinden
/// bir karakter alır: en büyük kutu `@`, en küçük kutu `.` olur. Böylece ızgara
/// yalnızca yoğunluk değil, kutuların **sınırlarını** da gösterir; aksi hâlde
/// benzer boyutlu kutular tek renkli bir blok olarak görünür ve harita hiçbir
/// bilgi taşımazdı. `RAMP`ın ilk karakteri (boşluk) yalnızca kutuların dışı
/// için ayrılmıştır.
///
/// Kutuların dışındaki hücreler boşluktur. Dönen metin tam olarak `yukseklik`
/// satır ve her satır tam olarak `genislik` karakter içerir; bu, test edilebilir
/// bir değişmezdir.
#[must_use]
pub fn ascii_cikti(kutular: &[Kutu], genislik: usize, yukseklik: usize) -> String {
    if genislik == 0 || yukseklik == 0 || kutular.is_empty() {
        return String::new();
    }
    let ham_en = kutular
        .iter()
        .map(|k| k.x + k.genislik)
        .fold(0.0_f64, f64::max);
    let ham_boy = kutular
        .iter()
        .map(|k| k.y + k.yukseklik)
        .fold(0.0_f64, f64::max);
    let en = ham_en.max(f64::MIN_POSITIVE);
    let boy = ham_boy.max(f64::MIN_POSITIVE);
    let olcek = (genislik as f64 / en).min(yukseklik as f64 / boy);

    // Alan sıralaması: büyükten küçüğe. Eşitlikte etiket sırası belirleyicidir,
    // böylece çıktı aynı girdiden her zaman aynıdır.
    let mut sira: Vec<usize> = (0..kutular.len()).collect();
    sira.sort_by(|a, b| {
        kutular[*b]
            .alan()
            .partial_cmp(&kutular[*a].alan())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| kutular[*a].etiket.cmp(&kutular[*b].etiket))
    });
    let adet = sira.len();
    let en_yogun = RAMP.len() - 1;
    let en_seyrek = 1usize;
    let mut isaret = vec![en_seyrek; adet];
    for (s, &i) in sira.iter().enumerate() {
        let adim = (s * (en_yogun - en_seyrek)) / adet.saturating_sub(1).max(1);
        isaret[i] = en_yogun - adim;
    }

    let mut cikti = String::with_capacity((genislik + 1) * yukseklik);
    for satir in 0..yukseklik {
        for sutun in 0..genislik {
            let mx = (sutun as f64 + 0.5) / olcek;
            let my = (satir as f64 + 0.5) / olcek;
            let bulunan = kutular.iter().position(|k| {
                mx >= k.x && mx < k.x + k.genislik && my >= k.y && my < k.y + k.yukseklik
            });
            match bulunan {
                None => cikti.push(' '),
                Some(i) => cikti.push(RAMP[isaret[i].min(en_yogun)] as char),
            }
        }
        cikti.push('\n');
    }
    cikti
}

#[cfg(test)]
mod tests {
    use super::*;

    fn girdiler(veri: &[(&str, u64)]) -> Vec<Girdi> {
        veri.iter().map(|(e, d)| Girdi::yeni(*e, *d)).collect()
    }

    #[test]
    fn bos_girdi_listesi_bos_kutu_dondurur() {
        assert!(yerlestir(&[], 0.0, 0.0, 100.0, 100.0).is_empty());
    }

    #[test]
    fn sifir_degerli_girdiler_yerlestirilmez() {
        let g = girdiler(&[("a", 0), ("b", 0)]);
        assert!(yerlestir(&g, 0.0, 0.0, 10.0, 10.0).is_empty());
    }

    #[test]
    fn tek_kutu_tum_alani_kaplar() {
        let g = girdiler(&[("tek", 500)]);
        let k = yerlestir(&g, 0.0, 0.0, 100.0, 50.0);
        assert_eq!(k.len(), 1);
        assert!((k[0].alan() - 5000.0).abs() < 1e-6);
    }

    #[test]
    fn sifir_genislik_gecersiz_alan_dondurur() {
        let g = girdiler(&[("a", 1)]);
        assert!(yerlestir(&g, 0.0, 0.0, 0.0, 10.0).is_empty());
        assert!(yerlestir(&g, 0.0, 0.0, 10.0, -1.0).is_empty());
    }

    #[test]
    fn kutu_alani_girdi_toplami_ile_orantili_durur() {
        let g = girdiler(&[("a", 50), ("b", 30), ("c", 20)]);
        let k = yerlestir(&g, 0.0, 0.0, 100.0, 100.0);
        let toplam_alan: f64 = k.iter().map(Kutu::alan).sum();
        let toplam_deger: u64 = k.iter().map(|x| x.deger).sum();
        assert!(
            (toplam_alan - 10_000.0).abs() < 1e-3,
            "toplam alan {toplam_alan}"
        );
        for b in &k {
            let beklenen = b.deger as f64 / toplam_deger as f64 * 10_000.0;
            assert!(
                (b.alan() - beklenen).abs() / beklenen < 0.01,
                "{} alanı {} beklenen {}",
                b.etiket,
                b.alan(),
                beklenen
            );
        }
    }

    #[test]
    fn kutular_dikdortgenin_icinde_kalir() {
        let g = girdiler(&[("a", 9), ("b", 1), ("c", 4), ("d", 6), ("e", 2)]);
        let k = yerlestir(&g, 0.0, 0.0, 120.0, 60.0);
        for b in &k {
            assert!(b.x >= -1e-9, "{} x={}", b.etiket, b.x);
            assert!(b.y >= -1e-9, "{} y={}", b.etiket, b.y);
            assert!(b.x + b.genislik <= 120.0 + 1e-6);
            assert!(b.y + b.yukseklik <= 60.0 + 1e-6);
        }
    }

    #[test]
    fn kutular_birbiriyle_kesismez() {
        let g = girdiler(&[("a", 9), ("b", 1), ("c", 4), ("d", 6), ("e", 2), ("f", 3)]);
        let k = yerlestir(&g, 0.0, 0.0, 100.0, 100.0);
        for (i, a) in k.iter().enumerate() {
            for b in k.iter().skip(i + 1) {
                let cakisma = a.x < b.x + b.genislik - 1e-9
                    && b.x < a.x + a.genislik - 1e-9
                    && a.y < b.y + b.yukseklik - 1e-9
                    && b.y < a.y + a.yukseklik - 1e-9;
                assert!(!cakisma, "{} ve {} kesisiyor", a.etiket, b.etiket);
            }
        }
    }

    #[test]
    fn en_boy_orani_kareye_yakin_kalir() {
        let g = girdiler(&[("a", 6), ("b", 6), ("c", 6), ("d", 6)]);
        let k = yerlestir(&g, 0.0, 0.0, 100.0, 100.0);
        for b in &k {
            let oran = b.genislik / b.yukseklik;
            assert!(
                (0.4..2.5).contains(&oran),
                "{} oranı {oran} kareye uzak",
                b.etiket
            );
        }
    }

    #[test]
    fn cok_kucuk_alan_yine_de_terminasyon_garanti_eder() {
        let g = girdiler(&[("a", 1_000_000), ("b", 1), ("c", 1), ("d", 1)]);
        let k = yerlestir(&g, 0.0, 0.0, 3.0, 3.0);
        assert_eq!(k.len(), 4);
    }

    #[test]
    fn yerlesim_girdi_sirasindan_bagimsizdir() {
        let a = girdiler(&[("a", 1), ("b", 2), ("c", 3)]);
        let b = girdiler(&[("c", 3), ("a", 1), ("b", 2)]);
        let ka = yerlestir(&a, 0.0, 0.0, 50.0, 50.0);
        let kb = yerlestir(&b, 0.0, 0.0, 50.0, 50.0);
        assert_eq!(ka.len(), kb.len());
        for x in &ka {
            let y = kb
                .iter()
                .find(|y| y.etiket == x.etiket)
                .expect("etiket eslesmeli");
            assert!((x.alan() - y.alan()).abs() < 1e-6);
        }
    }

    #[test]
    fn esit_degerlerde_etiket_sirasi_belirleyicidir() {
        let a = girdiler(&[("zeta", 5), ("alfa", 5), ("mid", 5)]);
        let sirali = yerlestir(&a, 0.0, 0.0, 30.0, 30.0);
        let etiketler: Vec<&str> = sirali.iter().map(|k| k.etiket.as_str()).collect();
        assert_eq!(etiketler, vec!["alfa", "mid", "zeta"]);
    }

    #[test]
    fn kotuluk_kare_yerlesimde_en_kucuk_deger() {
        // Tek kutu, kısa kenar 10, değer 100 -> 10x10 kare, kotuluk 1.
        let k = kotuluk(100.0, 1.0, 100.0, 100.0, 10.0);
        assert!((k - 1.0).abs() < 1e-9);
    }

    #[test]
    fn kotuluk_gecersiz_girdide_sonsuz_dondurur() {
        assert!(kotuluk(0.0, 1.0, 1.0, 1.0, 10.0).is_infinite());
        assert!(kotuluk(10.0, 0.0, 1.0, 1.0, 10.0).is_infinite());
    }

    #[test]
    fn kutu_merkezi_dogru_hesaplanir() {
        let k = Kutu {
            etiket: "a".to_string(),
            deger: 1,
            x: 10.0,
            y: 20.0,
            genislik: 4.0,
            yukseklik: 6.0,
        };
        assert_eq!(k.merkez(), (12.0, 23.0));
        assert!((k.alan() - 24.0).abs() < 1e-9);
    }

    #[test]
    fn ascii_cikti_boyutlari_tamdir() {
        let g = girdiler(&[("a", 90), ("b", 10)]);
        let k = yerlestir(&g, 0.0, 0.0, 100.0, 50.0);
        let c = ascii_cikti(&k, 40, 10);
        let satirlar: Vec<&str> = c.lines().collect();
        assert_eq!(satirlar.len(), 10);
        for s in satirlar {
            assert_eq!(s.chars().count(), 40, "satır genişliği 40 olmalı");
        }
    }

    #[test]
    fn ascii_cikti_deterministiktir() {
        let g = girdiler(&[("a", 5), ("b", 3), ("c", 2)]);
        let k = yerlestir(&g, 0.0, 0.0, 80.0, 40.0);
        assert_eq!(ascii_cikti(&k, 32, 8), ascii_cikti(&k, 32, 8));
    }

    #[test]
    fn ascii_cikti_bos_parametrelerde_bos_dondurur() {
        let k = yerlestir(&girdiler(&[("a", 1)]), 0.0, 0.0, 10.0, 10.0);
        assert_eq!(ascii_cikti(&k, 0, 5), "");
        assert_eq!(ascii_cikti(&k, 5, 0), "");
        assert_eq!(ascii_cikti(&[], 5, 5), "");
    }

    #[test]
    fn ascii_cikti_rampa_karakterlerinden_olusur() {
        let g = girdiler(&[("a", 99), ("b", 1)]);
        let k = yerlestir(&g, 0.0, 0.0, 20.0, 20.0);
        let c = ascii_cikti(&k, 20, 6);
        for ch in c.chars() {
            assert!(
                ch == '\n' || RAMP.contains(&(ch as u32 as u8)),
                "beklenmeyen karakter: {ch}"
            );
        }
    }

    #[test]
    fn ascii_cikti_en_buyuk_kutuya_en_yogun_karakteri_verir() {
        let g = girdiler(&[("kucuk", 1), ("buyuk", 900), ("orta", 50)]);
        let k = yerlestir(&g, 0.0, 0.0, 40.0, 40.0);
        let (genislik, yukseklik) = (40_usize, 20_usize);
        let c = ascii_cikti(&k, genislik, yukseklik);
        let buyuk = k
            .iter()
            .find(|b| b.etiket == "buyuk")
            .expect("buyuk kutu")
            .merkez();
        assert_eq!(hucre_karakteri(&c, &k, genislik, yukseklik, buyuk), '@');

        let kucuk = k
            .iter()
            .find(|b| b.etiket == "kucuk")
            .expect("kucuk kutu")
            .merkez();
        let yo = hucre_karakteri(&c, &k, genislik, yukseklik, kucuk);
        let yo_idx = RAMP.iter().position(|&x| x == yo as u8).unwrap_or(0);
        assert!(
            yo_idx < RAMP.len() - 1,
            "kucuk kutu buyuk kutudan daha yogun olmamali"
        );
    }

    #[test]
    fn ascii_cikti_kutu_sinirlarini_ayirt_edilebilir_kilar() {
        // Üç eşit olmayan kutu: en az iki farklı karakter görünmeli, aksi hâlde
        // ızgara tek blok olarak görünür ve bölüntü bilgisi kaybolur.
        let g = girdiler(&[("a", 600), ("b", 300), ("c", 100)]);
        let k = yerlestir(&g, 0.0, 0.0, 60.0, 30.0);
        let c = ascii_cikti(&k, 60, 30);
        let karakterler: std::collections::BTreeSet<char> =
            c.chars().filter(|ch| *ch != ' ' && *ch != '\n').collect();
        assert!(
            karakterler.len() >= 2,
            "kutular en az iki farkli karakterle ayirt edilebilmeli, gorulen: {karakterler:?}"
        );
    }

    /// Mantıksal bir koordinattaki hücrenin karakterini bağımsız olarak hesaplar.
    fn hucre_karakteri(
        cikti: &str,
        kutular: &[Kutu],
        genislik: usize,
        yukseklik: usize,
        (mx, my): (f64, f64),
    ) -> char {
        let en = kutular
            .iter()
            .map(|k| k.x + k.genislik)
            .fold(0.0_f64, f64::max)
            .max(f64::MIN_POSITIVE);
        let boy = kutular
            .iter()
            .map(|k| k.y + k.yukseklik)
            .fold(0.0_f64, f64::max)
            .max(f64::MIN_POSITIVE);
        let olcek = (genislik as f64 / en).min(yukseklik as f64 / boy);
        let sutun = ((mx * olcek) as usize).min(genislik - 1);
        let satir = ((my * olcek) as usize).min(yukseklik - 1);
        cikti
            .lines()
            .nth(satir)
            .and_then(|l| l.chars().nth(sutun))
            .unwrap_or(' ')
    }
}

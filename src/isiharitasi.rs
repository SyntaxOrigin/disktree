//! Isı haritası verisi: değişim büyüklüğü × derinlik skoru.
//!
//! Raporın ısı haritası fikri, "değişim büyüklüğü × derinlik skoru" çarpımını
//! bir görsel yoğunluğa çevirmektir. Grafik arayüz yasak olduğu için sonuç
//! metin olarak sıralı bir liste üretilir; ısı haritası verisi bu modülün
//! çıktısıdır.
//!
//! Derinlik skoru `1 / (1 + derinlik)` şeklindedir: sığ yüzeydeki değişimler
//! "sıcak", derine gömülü değişimler "soğuk" görünür. Bu seçim raporun
//! bölümleme amacıyla tutarlıdır — kökün altındaki şişkin bir klasör, kökün
//! kendisinden daha az ilginçtir.
//!
//! `seviye`, normalize edilmiş skora göre 0-4 arası bir yoğunluk kovasıdır ve
//! terminal çıktısında `.:-=+*#%@` rampasıyla gösterilir.

use crate::rapor::{IsiJson, Kayma};
use crate::zaman::fark;

/// Bir ısı haritası satırı.
#[derive(Debug, Clone, PartialEq)]
pub struct IsiSatir {
    /// Değişen yol.
    pub yol: String,
    /// İki tarama arasındaki boyut farkı.
    pub degisim: i64,
    /// Yolun derinliği.
    pub derinlik: u16,
    /// `1 / (1 + derinlik)`.
    pub derinlik_skoru: f64,
    /// `|degisim| * derinlik_skoru`.
    pub skor: f64,
    /// En yüksek skora göre normalize edilmiş `0..=1` değer.
    pub normalize: f64,
    /// `0..=4` yoğunluk seviyesi.
    pub seviye: u8,
}

/// Seviye etiketleri (soldan sağa soğuktan sıcağa).
const ETIKETLER: [&str; 5] = ["cok dusuk", "dusuk", "orta", "yuksek", "cok yuksek"];

/// İki anlık görüntüden ısı haritası verisi üretir.
///
/// Sonuç skora göre büyükten küçüğe sıralıdır. Yalnızca değişmiş yollar
/// listelenir; hiç değişiklik yoksa boş vektör döner.
#[must_use]
pub fn hesapla(onceki: &Kayma, sonraki: &Kayma) -> Vec<IsiSatir> {
    let derinlikler = derinlik_haritasi(onceki, sonraki);
    let mut satirlar: Vec<IsiSatir> = fark(onceki, sonraki)
        .into_iter()
        .map(|s| {
            let derinlik = derinlikler.get(&s.yol).copied().unwrap_or(1);
            let derinlik_skoru = 1.0 / (1.0 + f64::from(derinlik));
            let skor = (s.fark.unsigned_abs() as f64) * derinlik_skoru;
            IsiSatir {
                yol: s.yol,
                degisim: s.fark,
                derinlik,
                derinlik_skoru,
                skor,
                normalize: 0.0,
                seviye: 0,
            }
        })
        .collect();

    let en_buyuk = satirlar.iter().map(|s| s.skor).fold(0.0_f64, f64::max);
    for s in &mut satirlar {
        s.normalize = if en_buyuk > 0.0 {
            s.skor / en_buyuk
        } else {
            0.0
        };
        s.seviye = seviye(s.normalize);
    }
    satirlar.sort_by(|a, b| {
        b.skor
            .partial_cmp(&a.skor)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.yol.cmp(&a.yol))
    });
    satirlar
}

/// İki anlık görüntüde bulunan tüm yolların derinlik haritası.
fn derinlik_haritasi(onceki: &Kayma, sonraki: &Kayma) -> std::collections::HashMap<String, u16> {
    let mut harita = std::collections::HashMap::new();
    for k in [onceki, sonraki] {
        for d in &k.dugumler {
            harita.insert(d.ad.clone(), d.derinlik);
        }
    }
    harita
}

/// Normalize edilmiş skoru 0-4 arası kovaya çevirir.
fn seviye(normalize: f64) -> u8 {
    match normalize {
        n if n >= 0.85 => 4,
        n if n >= 0.60 => 3,
        n if n >= 0.35 => 2,
        n if n >= 0.10 => 1,
        _ => 0,
    }
}

/// Isı satırının seviye etiketini döndürür.
#[must_use]
pub fn etiket(seviye_no: u8) -> &'static str {
    ETIKETLER
        .get(seviye_no as usize)
        .copied()
        .unwrap_or(ETIKETLER[0])
}

/// Isı satırlarını JSON çıktısına çevirir.
#[must_use]
pub fn jsonla(satirlar: &[IsiSatir]) -> Vec<IsiJson> {
    satirlar
        .iter()
        .map(|s| IsiJson {
            yol: s.yol.clone(),
            degisim: s.degisim,
            derinlik: s.derinlik,
            derinlik_skoru: s.derinlik_skoru,
            skor: s.skor,
            normalize: s.normalize,
            seviye: s.seviye,
            etiket: etiket(s.seviye).to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rapor::DugumJson;
    use crate::zaman::fark as zaman_fark;

    fn dugum(yol: &str, alt: u64, derinlik: u16) -> DugumJson {
        DugumJson {
            ad: yol.to_string(),
            tur: "dizin".to_string(),
            boyt: 0,
            alt_toplam: alt,
            derinlik,
            degistirme: 0,
            gizli: false,
            sistem: false,
        }
    }

    fn kayma(d: Vec<DugumJson>) -> Kayma {
        Kayma {
            bicim: crate::rapor::BICIM.to_string(),
            surum: 1,
            kok: "/kok".to_string(),
            zaman: 0,
            dugumler: d,
        }
    }

    #[test]
    fn derinlik_skoru_azalan_bir_dizidir() {
        let o = kayma(vec![dugum("/kok/sig", 0, 1), dugum("/kok/derin/a/b", 0, 4)]);
        let s = kayma(vec![
            dugum("/kok/sig", 100, 1),
            dugum("/kok/derin/a/b", 100, 4),
        ]);
        let satirlar = hesapla(&o, &s);
        let sig = satirlar
            .iter()
            .find(|x| x.yol == "/kok/sig")
            .expect("sığ yol");
        let derin = satirlar
            .iter()
            .find(|x| x.yol == "/kok/derin/a/b")
            .expect("derin yol");
        assert!(
            sig.skor > derin.skor,
            "aynı değişimde sığ yol daha sıcak olmalı"
        );
        assert!((sig.derinlik_skoru - 0.5).abs() < 1e-9);
        assert!((derin.derinlik_skoru - 0.2).abs() < 1e-9);
    }

    #[test]
    fn is_ham_gidis_donusu_durum_etiketi_durur() {
        let o = kayma(vec![dugum("/kok/a", 0, 1)]);
        let s = kayma(vec![dugum("/kok/a", 50, 1)]);
        let i = hesapla(&o, &s);
        assert_eq!(i[0].degisim, 50);
        assert_eq!(i.len(), zaman_fark(&o, &s).len());
    }

    #[test]
    fn hicbir_degisiklik_yoksa_bos_liste_dondurur() {
        let k = kayma(vec![dugum("/kok/a", 10, 1)]);
        assert!(hesapla(&k, &k).is_empty());
    }

    #[test]
    fn normalize_en_buyuk_skoru_bire_yapar() {
        let o = kayma(vec![dugum("/kok/a", 0, 1), dugum("/kok/b", 0, 1)]);
        let s = kayma(vec![dugum("/kok/a", 500, 1), dugum("/kok/b", 10, 1)]);
        let i = hesapla(&o, &s);
        assert_eq!(i[0].yol, "/kok/a");
        assert!((i[0].normalize - 1.0).abs() < 1e-9);
        assert!((i[1].normalize - 0.02).abs() < 1e-9);
    }

    #[test]
    fn seviye_esikleri_artan_siradir() {
        assert_eq!(seviye(0.0), 0);
        assert_eq!(seviye(0.2), 1);
        assert_eq!(seviye(0.4), 2);
        assert_eq!(seviye(0.7), 3);
        assert_eq!(seviye(0.9), 4);
    }

    #[test]
    fn etiketler_soğuktan_sicağa_siralidir() {
        assert_eq!(etiket(0), "cok dusuk");
        assert_eq!(etiket(4), "cok yuksek");
        assert_eq!(etiket(9), "cok dusuk");
    }

    #[test]
    fn sonuclar_skora_gore_azalan_sirali_durur() {
        let o = kayma(vec![
            dugum("/kok/a", 0, 1),
            dugum("/kok/b", 0, 1),
            dugum("/kok/c", 0, 1),
        ]);
        let s = kayma(vec![
            dugum("/kok/a", 1000, 1),
            dugum("/kok/b", 50, 1),
            dugum("/kok/c", 500, 1),
        ]);
        let i = hesapla(&o, &s);
        assert!(i.windows(2).all(|w| w[0].skor >= w[1].skor));
    }

    #[test]
    fn json_donusturme_alanlari_korur() {
        let o = kayma(vec![dugum("/kok/a", 0, 1)]);
        let s = kayma(vec![dugum("/kok/a", 900, 1)]);
        let j = jsonla(&hesapla(&o, &s));
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].yol, "/kok/a");
        assert_eq!(j[0].seviye, 4);
        assert_eq!(j[0].etiket, "cok yuksek");
        assert!(j[0].derinlik_skoru > 0.0);
    }
}

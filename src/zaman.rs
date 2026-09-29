//! Boyut zaman çizelgesi: aylık kümülatif boyut eğrisi ve iki taramayı karşılaştırma.
//!
//! Zaman crate'i (`chrono`, `time`) bağımlılık politikasıyla yasaktır; takvim
//! dönüşümü Howard Hinnant'ın `civil_from_days` algoritmasıyla elle yapılır.
//! Bu algoritma 1 Ocak 1970'den bu yana geçen gün sayısını, yıl/ay/gün
//! üçlüsüne kayıpsız çevirir ve testlerde bilinen tarihlerle doğrulanır.
//!
//! "Hangi klasör ne zaman büyüdü" sorusu iki kaynaktan yanıtlanır:
//!
//! * [`aylik_kume`] — dosya son değişiklik tarihine göre aylık kümelenmiş,
//!   kümülatif büyüme eğrisi (raporun `b05` v1 satırı).
//! * [`fark`] — iki taramanın yol bazlı farkı; hangi yolun ne kadar
//!   büyüdüğünü/ küçüldüğünü döndürür.

use crate::hata::Hata;
use crate::rapor::{DugumJson, Kayma};
use std::collections::HashMap;

/// Bir saniye ile bir gün arasındaki geçen saniye sayısı.
const GUN_SANIYE: i64 = 86_400;

/// `1969-12-31`'den 1 Ocak 1970'e kadar eksi bir gün.
const CEVRIM_BASLANGIC: i64 = 719_468;

/// Gün sayısından `(yıl, ay, gün)` üçlüsüne çevirir (ay 1-12, gün 1-31).
#[must_use]
pub fn gun_sayisindan_tarihe(gun: i64) -> (i64, u32, u32) {
    let z = gun + CEVRIM_BASLANGIC;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let g = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let ay = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if ay <= 2 { y + 1 } else { y }, ay, g)
}

/// `(yıl, ay, gün)` üçlüsünden 1970-01-01'e göre gün sayısını üretir.
#[must_use]
pub fn tarihten_gun_sayisina(yil: i64, ay: u32, gun: u32) -> i64 {
    let m = i64::from(ay);
    let d = i64::from(gun);
    let y = if m <= 2 { yil - 1 } else { yil };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - CEVRIM_BASLANGIC
}

/// Unix epoch saniyesini `YYYY-AA` biçiminde aya çevirir.
#[must_use]
pub fn ay_etiketi(epoch_saniye: u64) -> String {
    let gun = (epoch_saniye as i64).div_euclid(GUN_SANIYE);
    let (yil, ay, _) = gun_sayisindan_tarihe(gun);
    format!("{yil:04}-{ay:02}")
}

/// Unix epoch saniyesini `(yıl, ay)` biçiminde aya çevirir.
#[must_use]
pub fn ay_yil(epoch_saniye: u64) -> (i64, u32) {
    let gun = (epoch_saniye as i64).div_euclid(GUN_SANIYE);
    let (yil, ay, _) = gun_sayisindan_tarihe(gun);
    (yil, ay)
}

/// Aylık kümelenmiş, kümülatif büyüme eğrisinin tek bir noktası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AyKutusu {
    /// `YYYY-AA` biçiminde ay etiketi.
    pub ay: String,
    /// O ayda son değişiklik geçirmiş dosyaların toplam baytı.
    pub bayt: u64,
    /// O ayda sayılan dosya sayısı.
    pub dosya: u32,
    /// Aylar arasında kümülatif toplam bayt.
    pub kumulatif_bayt: u64,
    /// Aylar arasında kümülatif dosya sayısı.
    pub kumulatif_dosya: u64,
}

/// Bir tarama anlık görüntüsünden aylık kümülatif büyüme eğrisi üretir.
///
/// Aylar artan sırada döner; hiçbir dosyanın zaman damgası yoksa (`0`)
/// `1970-01` kovasına konur, çünkü silmek yerine kayıp olarak görünmesi
/// daha dürüst bir davranıştır.
#[must_use]
pub fn aylik_kume(kayma: &Kayma) -> Vec<AyKutusu> {
    let mut kova: HashMap<String, (u64, u32)> = HashMap::new();
    for d in &kayma.dugumler {
        if d.tur != crate::model::Tur::Dosya.ad() {
            continue;
        }
        let e = kova.entry(ay_etiketi(d.degistirme)).or_insert((0, 0));
        e.0 = e.0.saturating_add(d.boyt);
        e.1 = e.1.saturating_add(1);
    }
    let mut sirali: Vec<AyKutusu> = kova
        .into_iter()
        .map(|(ay, (bayt, dosya))| AyKutusu {
            ay,
            bayt,
            dosya,
            kumulatif_bayt: 0,
            kumulatif_dosya: 0,
        })
        .collect();
    sirali.sort_by(|a, b| a.ay.cmp(&b.ay));
    let mut k_bayt = 0_u64;
    let mut k_dosya = 0_u64;
    for kutu in &mut sirali {
        k_bayt = k_bayt.saturating_add(kutu.bayt);
        k_dosya = k_dosya.saturating_add(u64::from(kutu.dosya));
        kutu.kumulatif_bayt = k_bayt;
        kutu.kumulatif_dosya = k_dosya;
    }
    sirali
}

/// İki tarama arasındaki yol bazlı boyut farkı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZamanSatiri {
    /// Değişen yolun tam yolu.
    pub yol: String,
    /// Önceki taramadaki alt toplam.
    pub onceki_bayt: u64,
    /// Yeni taramadaki alt toplam.
    pub sonraki_bayt: u64,
    /// `sonraki - onceki` farkı (negatif olabilir).
    pub fark: i64,
    /// Değişim sırası numarası (1 = en çok büyüyen).
    pub sira: u32,
    /// Satırın niteliği: `yeni`, `silinmis`, `buyudu`, `kuculdu`, `degismedi`.
    pub durum: &'static str,
}

/// İki anlık görüntüyü yol bazında karşılaştırır ve farkı büyüklüğüne göre sıralar.
///
/// Sonuç yalnızca **değişen** yolları içerir; hiç değişiklik yoksa boş vektör
/// döner. Farkı sıfır olan yollar "degismedi" olarak **atılır**, çünkü zaman
/// çizelgesinin amacı büyümeyi göstermektir.
#[must_use]
pub fn fark(onceki: &Kayma, sonraki: &Kayma) -> Vec<ZamanSatiri> {
    let eski: HashMap<&str, u64> = onceki
        .dugumler
        .iter()
        .map(|d| (d.ad.as_str(), d.alt_toplam))
        .collect();
    let mut satirlar: Vec<ZamanSatiri> = Vec::new();

    for d in &sonraki.dugumler {
        let o = eski.get(d.ad.as_str()).copied();
        let f = i64::try_from(d.alt_toplam)
            .unwrap_or(i64::MAX)
            .saturating_sub(i64::try_from(o.unwrap_or(0)).unwrap_or(0));
        if f == 0 && o.is_some() {
            continue;
        }
        satirlar.push(ZamanSatiri {
            yol: d.ad.clone(),
            onceki_bayt: o.unwrap_or(0),
            sonraki_bayt: d.alt_toplam,
            fark: f,
            sira: 0,
            durum: durum_belirle(o, Some(&d.alt_toplam), f),
        });
    }
    for d in &onceki.dugumler {
        if sonraki.dugumler.iter().any(|y| y.ad == d.ad) {
            continue;
        }
        satirlar.push(ZamanSatiri {
            yol: d.ad.clone(),
            onceki_bayt: d.alt_toplam,
            sonraki_bayt: 0,
            fark: -i64::try_from(d.alt_toplam).unwrap_or(i64::MAX),
            sira: 0,
            durum: "silinmis",
        });
    }

    satirlar.sort_by(|a, b| {
        b.fark
            .abs()
            .cmp(&a.fark.abs())
            .then_with(|| b.yol.cmp(&a.yol))
    });
    for (i, s) in satirlar.iter_mut().enumerate() {
        s.sira = u32::try_from(i + 1).unwrap_or(u32::MAX);
    }
    satirlar
}

fn durum_belirle(onceki: Option<u64>, sonraki: Option<&u64>, fark: i64) -> &'static str {
    if onceki.is_none() {
        return "yeni";
    }
    if sonraki.is_none() {
        return "silinmis";
    }
    if fark > 0 {
        "buyudu"
    } else if fark < 0 {
        "kuculdu"
    } else {
        "degismedi"
    }
}

/// Bir anlık görüntüdeki yol listesini döndürür (ağaç yerine düz liste).
#[must_use]
pub fn yollar(kayma: &Kayma) -> Vec<&DugumJson> {
    kayma.dugumler.iter().collect()
}

/// İki anlık görüntünün biçim/sürüm uyumunu doğrular.
///
/// # Hatalar
///
/// Sürüm damgası beklenenden farklıysa [`Hata::KaymaSurusuUyumsuz`] döner.
pub fn surum_dogrula(kayma: &Kayma, beklenen: u32) -> Result<(), Hata> {
    if kayma.surum != beklenen {
        return Err(Hata::KaymaSurusuUyumsuz {
            bulunan: kayma.surum,
            beklenen,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rapor::BICIM;

    fn dugum(yol: &str, tur: &str, boyt: u64, alt: u64, zaman: u64) -> DugumJson {
        DugumJson {
            ad: yol.to_string(),
            tur: tur.to_string(),
            boyt,
            alt_toplam: alt,
            derinlik: 1,
            degistirme: zaman,
            gizli: false,
            sistem: false,
        }
    }

    fn kayma(d: Vec<DugumJson>) -> Kayma {
        Kayma {
            bicim: BICIM.to_string(),
            surum: 1,
            kok: "/kok".to_string(),
            zaman: 0,
            dugumler: d,
        }
    }

    const OCAK_1970: u64 = 0;
    const MART_2000: u64 = 951_868_800;
    const EYLUL_2026: u64 = 1_790_640_000;

    #[test]
    fn gun_sayisi_ve_tarih_arasindaki_gidis_donusu_kayipsizdir() {
        for &(y, a, g) in &[(1970, 1, 1), (2000, 3, 1), (2026, 9, 29), (1999, 12, 31)] {
            let gun = tarihten_gun_sayisina(y, a, g);
            assert_eq!(gun_sayisindan_tarihe(gun), (y, a, g), "{y}-{a}-{g}");
        }
    }

    #[test]
    fn bilinen_tarihler_dogru_gun_sayisi_verir() {
        assert_eq!(tarihten_gun_sayisina(1970, 1, 1), 0);
        assert_eq!(tarihten_gun_sayisina(1970, 1, 2), 1);
        assert_eq!(tarihten_gun_sayisina(2000, 3, 1), 11_017);
        assert_eq!(tarihten_gun_sayisina(2026, 9, 29), 20_725);
    }

    #[test]
    fn epoch_sifiri_ocak_1970_ayini_verir() {
        assert_eq!(ay_etiketi(OCAK_1970), "1970-01");
        assert_eq!(ay_yil(OCAK_1970), (1970, 1));
    }

    #[test]
    fn gun_sinirinda_ay_degismez_ve_ay_sinirinda_degisir() {
        // 1970 Ocak ayının son saniyesi hâlâ 1970-01, ilk saniye 1970-02.
        assert_eq!(ay_etiketi(86_399), "1970-01");
        assert_eq!(ay_etiketi(2_678_399), "1970-01");
        assert_eq!(ay_etiketi(2_678_400), "1970-02");
        assert_eq!(ay_etiketi(5_159_200), "1970-03");
    }

    #[test]
    fn mart_2000_ve_eylul_2026_ay_etiketleri_dogru() {
        assert_eq!(ay_etiketi(MART_2000), "2000-03");
        assert_eq!(ay_etiketi(EYLUL_2026), "2026-09");
    }

    #[test]
    fn aylik_kume_yalnizca_dosyalari_sayar() {
        let k = kayma(vec![
            dugum("/kok/a.txt", "dosya", 100, 100, MART_2000),
            dugum("/kok/klasor", "dizin", 0, 100, MART_2000),
        ]);
        let aylar = aylik_kume(&k);
        assert_eq!(aylar.len(), 1);
        assert_eq!(aylar[0].bayt, 100);
        assert_eq!(aylar[0].dosya, 1);
    }

    #[test]
    fn aylik_kume_kumulatif_toplam_verir() {
        let k = kayma(vec![
            dugum("/kok/a.txt", "dosya", 10, 10, MART_2000),
            dugum("/kok/b.txt", "dosya", 25, 25, EYLUL_2026),
        ]);
        let aylar = aylik_kume(&k);
        assert_eq!(aylar.len(), 2);
        assert_eq!(aylar[0].ay, "2000-03");
        assert_eq!(aylar[0].kumulatif_bayt, 10);
        assert_eq!(aylar[1].ay, "2026-09");
        assert_eq!(aylar[1].kumulatif_bayt, 35);
        assert_eq!(aylar[1].kumulatif_dosya, 2);
    }

    #[test]
    fn aylik_kume_zamansiz_dosyayi_1970_ocak_kovasina_koyar() {
        let k = kayma(vec![dugum("/kok/a.txt", "dosya", 7, 7, 0)]);
        let aylar = aylik_kume(&k);
        assert_eq!(aylar[0].ay, "1970-01");
    }

    #[test]
    fn fark_buyuyen_klasoru_ilk_siraya_koyar() {
        let o = kayma(vec![
            dugum("/kok/kucuk", "dizin", 0, 10, 0),
            dugum("/kok/buyuk", "dizin", 0, 20, 0),
        ]);
        let s = kayma(vec![
            dugum("/kok/kucuk", "dizin", 0, 15, 0),
            dugum("/kok/buyuk", "dizin", 0, 100, 0),
        ]);
        let f = fark(&o, &s);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].yol, "/kok/buyuk");
        assert_eq!(f[0].fark, 80);
        assert_eq!(f[0].sira, 1);
        assert_eq!(f[0].durum, "buyudu");
        assert_eq!(f[1].yol, "/kok/kucuk");
        assert_eq!(f[1].sira, 2);
    }

    #[test]
    fn fark_yeni_ve_silinmis_yollari_isaretler() {
        let o = kayma(vec![dugum("/kok/oldu", "dosya", 5, 5, 0)]);
        let s = kayma(vec![dugum("/kok/yeni", "dosya", 3, 3, 0)]);
        let f = fark(&o, &s);
        let durumlar: Vec<&str> = f.iter().map(|x| x.durum).collect();
        assert!(durumlar.contains(&"yeni"));
        assert!(durumlar.contains(&"silinmis"));
    }

    #[test]
    fn fark_degismeyen_yollari_listelemez() {
        let k = kayma(vec![dugum("/kok/a", "dosya", 5, 5, 0)]);
        assert!(fark(&k, &k).is_empty());
    }

    #[test]
    fn fark_kuculen_yolu_negatif_isaretler() {
        let o = kayma(vec![dugum("/kok/a", "dosya", 100, 100, 0)]);
        let s = kayma(vec![dugum("/kok/a", "dosya", 40, 40, 0)]);
        let f = fark(&o, &s);
        assert_eq!(f[0].fark, -60);
        assert_eq!(f[0].durum, "kuculdu");
    }

    #[test]
    fn fark_artan_ve_kucunen_yollari_degistirilmeden_ayirir() {
        let o = kayma(vec![
            dugum("/kok/artan", "dizin", 0, 1_000_000, 0),
            dugum("/kok/azalan", "dizin", 0, 1_000_000, 0),
        ]);
        let s = kayma(vec![
            dugum("/kok/artan", "dizin", 0, 1_000_100, 0),
            dugum("/kok/azalan", "dizin", 0, 900_000, 0),
        ]);
        let f = fark(&o, &s);
        assert_eq!(f[0].yol, "/kok/azalan");
        assert_eq!(f[1].yol, "/kok/artan");
    }

    #[test]
    fn surum_dogrulama_uyumsuzlukta_hata_dondurur() {
        let mut k = kayma(Vec::new());
        k.surum = 7;
        assert!(surum_dogrula(&k, 1).is_err());
        k.surum = 1;
        assert!(surum_dogrula(&k, 1).is_ok());
    }

    #[test]
    fn yollar_listesi_dugumleri_ayni_sirayla_verir() {
        let k = kayma(vec![
            dugum("/kok/a", "dosya", 1, 1, 0),
            dugum("/kok/b", "dosya", 1, 1, 0),
        ]);
        let liste = yollar(&k);
        assert_eq!(liste.len(), 2);
        assert_eq!(liste[0].ad, "/kok/a");
    }
}

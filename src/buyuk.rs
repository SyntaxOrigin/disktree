//! Büyük, eskimiş ve şüpheli gizli dosya listesi.
//!
//! Raporun `b05` v1 satırı: "eşik tabanlı filtreler; sistem ve gizli
//! öznitelikli dosyalar ayrı işaretlenir. Her satır yol, boyut, son erişim ve
//! gerekçe sütununu taşır."
//!
//! **Bu modül hiçbir dosyayı silmeyi, taşımayı veya değiştirmeyi önermez.**
//! Yalnızca listeler. "Şüpheli" etiketi istatistiksel bir gözlemdir (boyut +
//! yaş + gizlilik bileşimi), bir kötücül yazılım hükmü değildir; bu ayrım
//! raporun `b10` bölümündeki uyarının aynısıdır.

use crate::model::{Agac, Tur};
use crate::rapor::BuyukSatirJson;
use std::path::Path;

/// Arşiv dosyası sayılan uzantılar (küçük harf, noktasız).
pub const ARSIV_UZANTILARI: [&str; 10] = [
    "zip", "7z", "rar", "tar", "gz", "bz2", "xz", "zst", "iso", "cab",
];

/// Bir günün saniye cinsinden uzunluğu.
const GUN_SANIYE: u64 = 86_400;

/// Listeleme ayarları.
#[derive(Debug, Clone)]
pub struct BuyuraSecenegi {
    /// Sadece bu eşiğin üzerindeki dosyalar listelenir.
    pub esik_bayt: u64,
    /// Sadece bu yaşın üzerindeki dosyalar listelenir.
    pub min_yas_gun: u64,
    /// Yaş hesabının referans noktası (Unix epoch saniyesi).
    ///
    /// Testlerin deterministik olması için bu değer **dışarıdan verilir**;
    /// CLI'de `SystemTime::now()` ile doldurulur.
    pub artik_zaman: u64,
    /// Arşiv uzantılı dosyalar listeden çıkarılsın mı.
    pub arsivleri_haric_et: bool,
    /// Gizli/sistem öznitelikli dosyalar listeye dahil edilsin mi.
    pub gizlileri_dahil_et: bool,
    /// En fazla kaç satır döndürülsün.
    pub en_fazla: usize,
}

impl Default for BuyuraSecenegi {
    fn default() -> Self {
        Self {
            esik_bayt: 500 * 1024 * 1024,
            min_yas_gun: 365,
            artik_zaman: 0,
            arsivleri_haric_et: true,
            gizlileri_dahil_et: true,
            en_fazla: 100,
        }
    }
}

/// Listelenen tek bir dosya satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyukSatir {
    /// Dosyanın tam yolu.
    pub yol: String,
    /// Bayt cinsinden boyut.
    pub bayt: u64,
    /// Son değişiklik zamanı (Unix epoch saniyesi).
    pub degistirme: u64,
    /// Artık zamana göre yaş (gün).
    pub yas_gun: u64,
    /// Gizli özniteliği.
    pub gizli: bool,
    /// Sistem özniteliği.
    pub sistem: bool,
    /// Arşiv uzantısı mı.
    pub arsiv: bool,
    /// Kullanıcıya gösterilen gerekçe metni.
    pub gerekce: String,
}

impl BuyukSatir {
    /// JSON çıktısına çevirir.
    #[must_use]
    pub fn jsonla(&self) -> BuyukSatirJson {
        BuyukSatirJson {
            yol: self.yol.clone(),
            bayt: self.bayt,
            degistirme: self.degistirme,
            yas_gun: self.yas_gun,
            gizli: self.gizli,
            sistem: self.sistem,
            arsiv: self.arsiv,
            gerekce: self.gerekce.clone(),
        }
    }
}

/// Bir yolun uzantısı arşiv uzantısı mı?
#[must_use]
pub fn arsiv_mi(yol: &str) -> bool {
    Path::new(yol)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|u| ARSIV_UZANTILARI.contains(&u.as_str()))
}

/// Ağacı verilen ayarlarla süzer ve boyut azalan sırada listeler.
///
/// Sıralama tamamen deterministiktir: önce boyut azalan, sonra yol artan.
/// Eşitlik durumunda liste sırası platformdan bağımsızdır.
#[must_use]
pub fn listele(agac: &Agac, secenek: &BuyuraSecenegi) -> Vec<BuyukSatir> {
    let en_eski = secenek
        .artik_zaman
        .saturating_sub(secenek.min_yas_gun.saturating_mul(GUN_SANIYE));
    let mut satirlar: Vec<BuyukSatir> = agac
        .on_sira()
        .into_iter()
        .filter_map(|id| {
            let k = agac.kayit(id).ok()?;
            if k.tur != Tur::Dosya || k.boyut < secenek.esik_bayt {
                return None;
            }
            if k.degistirme > en_eski {
                return None;
            }
            if !secenek.gizlileri_dahil_et && (k.gizli || k.sistem) {
                return None;
            }
            let yol = agac.yol(id).ok()?.to_string();
            let arsiv = arsiv_mi(&yol);
            if arsiv && secenek.arsivleri_haric_et {
                return None;
            }
            let yas_gun = if k.degistirme >= secenek.artik_zaman {
                0
            } else {
                (secenek.artik_zaman - k.degistirme) / GUN_SANIYE
            };
            Some(BuyukSatir {
                gerekce: gerekce(k.gizli, k.sistem, arsiv, yas_gun, secenek.min_yas_gun),
                yol,
                bayt: k.boyut,
                degistirme: k.degistirme,
                yas_gun,
                gizli: k.gizli,
                sistem: k.sistem,
                arsiv,
            })
        })
        .collect();
    satirlar.sort_by(|a, b| b.bayt.cmp(&a.bayt).then_with(|| a.yol.cmp(&b.yol)));
    satirlar.truncate(secenek.en_fazla);
    satirlar
}

/// Bir satırın gerekçe metnini üretir.
fn gerekce(gizli: bool, sistem: bool, arsiv: bool, yas_gun: u64, min_yas: u64) -> String {
    let mut nedenler: Vec<&str> = Vec::new();
    if gizli {
        nedenler.push("gizli onitelikli");
    }
    if sistem {
        nedenler.push("sistem onitelikli");
    }
    if arsiv {
        nedenler.push("arsiv uzantili");
    }
    if yas_gun >= min_yas {
        nedenler.push("eski");
    }
    if nedenler.is_empty() {
        "esik ustu".to_string()
    } else {
        format!(
            "{} — bu bir istatistiksel gozlemdir, kotucul yazilim huku mu degildir; \
             arac hicbir dosyayi silmez veya degistirmez",
            nedenler.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DuzumGirdi;

    fn ornek_agac() -> Agac {
        let mut a = Agac::yeni("/kok").expect("agac");
        a.ekle(a.kok(), "/kok/buyuk.bin", DuzumGirdi::dosya(900, 1_000))
            .expect("buyuk");
        a.ekle(a.kok(), "/kok/kucuk.bin", DuzumGirdi::dosya(10, 1_000))
            .expect("kucuk");
        a.ekle(
            a.kok(),
            "/kok/yeni.bin",
            DuzumGirdi::dosya(900, 9_999_999_999),
        )
        .expect("yeni");
        a.ozeti_yeniden_hesapla();
        a
    }

    fn secenek() -> BuyuraSecenegi {
        BuyuraSecenegi {
            esik_bayt: 100,
            min_yas_gun: 0,
            artik_zaman: 10_000,
            ..BuyuraSecenegi::default()
        }
    }

    #[test]
    fn buyuk_dosyalar_esige_gore_suzulur() {
        let l = listele(&ornek_agac(), &secenek());
        assert_eq!(l.len(), 1, "yalnizca buyuk.bin esigi gecmeli");
        assert!(l[0].yol.ends_with("buyuk.bin"));
        assert!(!l.iter().any(|s| s.yol.ends_with("kucuk.bin")));
    }

    #[test]
    fn liste_boyuta_gore_azalan_siralidir() {
        let l = listele(&ornek_agac(), &secenek());
        assert!(l.windows(2).all(|w| w[0].bayt >= w[1].bayt));
    }

    #[test]
    fn yas_filtresi_guncel_dosyalari_eler() {
        let s = BuyuraSecenegi {
            min_yas_gun: 10,
            artik_zaman: 10_000,
            ..secenek()
        };
        let l = listele(&ornek_agac(), &s);
        assert!(l.iter().all(|x| x.yas_gun >= 10));
        assert!(!l.iter().any(|x| x.yol.ends_with("yeni.bin")));
    }

    #[test]
    fn arsiv_dosyalari_varsayilan_olarak_dislanir() {
        let mut a = ornek_agac();
        a.ekle(a.kok(), "/kok/yedek.zip", DuzumGirdi::dosya(5_000, 1_000))
            .expect("zip");
        a.ozeti_yeniden_hesapla();
        assert!(!listele(&a, &secenek())
            .iter()
            .any(|x| x.yol.ends_with(".zip")));

        let acik = BuyuraSecenegi {
            arsivleri_haric_et: false,
            ..secenek()
        };
        let l = listele(&a, &acik);
        assert!(l.iter().any(|x| x.yol.ends_with(".zip") && x.arsiv));
    }

    #[test]
    fn arsiv_uzantisi_tespiti_buyuk_kucuk_harf_duyarsizdir() {
        assert!(arsiv_mi("/a/b.ZIP"));
        assert!(arsiv_mi("/a/b.7z"));
        assert!(arsiv_mi("C:\\x\\y.iso"));
        assert!(!arsiv_mi("/a/b.txt"));
        assert!(!arsiv_mi("/a/yok"));
    }

    #[test]
    fn en_fazla_siniri_uygulanir() {
        let s = BuyuraSecenegi {
            en_fazla: 1,
            ..secenek()
        };
        assert_eq!(listele(&ornek_agac(), &s).len(), 1);
    }

    #[test]
    fn gizli_dosyalar_istenince_dahil_edilir() {
        let mut a = Agac::yeni("/kok").expect("agac");
        a.ekle(
            a.kok(),
            "/kok/.gizli",
            DuzumGirdi::dosya(500, 1_000).gizli(),
        )
        .expect("gizli");
        a.ozeti_yeniden_hesapla();
        assert_eq!(listele(&a, &secenek()).len(), 1);
        let kapali = BuyuraSecenegi {
            gizlileri_dahil_et: false,
            ..secenek()
        };
        assert!(listele(&a, &kapali).is_empty());
    }

    #[test]
    fn gerekce_metin_istatistiksel_gozlem_uyarisini_icerir() {
        let g = gerekce(true, false, false, 400, 365);
        assert!(g.contains("gizli"));
        assert!(g.contains("istatistiksel"));
        assert!(g.contains("silmez"));
        assert_eq!(gerekce(false, false, false, 0, 365), "esik ustu");
    }

    #[test]
    fn yas_hesabi_artan_zamanda_sifirlanir() {
        let mut a = Agac::yeni("/kok").expect("agac");
        // degistirme == artik_zaman: yas tam 0 gundur.
        a.ekle(a.kok(), "/kok/simdi.bin", DuzumGirdi::dosya(500, 10_000))
            .expect("simdi");
        a.ozeti_yeniden_hesapla();
        let l = listele(&a, &secenek());
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].yas_gun, 0);
    }

    #[test]
    fn json_donusturme_satiri_saklar() {
        let l = listele(&ornek_agac(), &secenek());
        let j = l[0].jsonla();
        assert_eq!(j.yol, l[0].yol);
        assert_eq!(j.bayt, l[0].bayt);
        assert!(!j.gerekce.is_empty());
    }

    #[test]
    fn varsayilan_esik_500_mib_ve_yas_365_gundur() {
        let s = BuyuraSecenegi::default();
        assert_eq!(s.esik_bayt, 524_288_000);
        assert_eq!(s.min_yas_gun, 365);
        assert!(s.arsivleri_haric_et);
        assert_eq!(s.en_fazla, 100);
    }

    #[test]
    fn dizinler_listeye_girmez() {
        let mut a = Agac::yeni("/kok").expect("agac");
        let d = a
            .ekle(a.kok(), "/kok/buyuk-dizin", DuzumGirdi::dizin(1_000))
            .expect("dizin");
        a.ekle(d, "/kok/buyuk-dizin/ic.bin", DuzumGirdi::dosya(500, 1_000))
            .expect("ic");
        a.ozeti_yeniden_hesapla();
        let l = listele(&a, &secenek());
        assert_eq!(l.len(), 1);
        assert!(l[0].yol.ends_with("ic.bin"));
    }
}

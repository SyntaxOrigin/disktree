//! Ayırma denetimi ("siyah kutu"): kayıp + küme alan tahmini.
//!
//! Raporun `b03` S3 senaryosu ve `b05` v1 satırı, `birim kapasitesi = dosya
//! verisi + yedeklenmiş kümeler + boş küme + kayıp` denklemini bir "siyah kutu"
//! satırı olarak göstermeyi ister.
//!
//! **Dürüstlük notu (rapor sapması).** Denklemin *ölçülebilir* kalan iki
//! terimi (`yedeklenmis`, `bos`) NTFS denetim bilgisinden okunur; DiskTree MFT
//! okumadığı için bu değerleri **kullanıcıdan alır**. `kayip` ise geriye kalan
//! farktır ve bir **tahmindir**, ölçüm değildir. Negatif çıkarsa bu, "veri
//! yok, model eksik" demektir ve saklamak yerine açıkça bildirilir.

use crate::model::{Agac, Tur};
use crate::rapor::AyirmaJson;

/// Ayırma denetimi girdileri.
#[derive(Debug, Clone, Copy)]
pub struct AyirmaGirdi {
    /// Birimin toplam kapasitesi (bayt).
    pub kapasite: u64,
    /// Dosya sistemi tarafından bildirilen boş alan (bayt).
    pub bos: u64,
    /// Yedeklenmiş küre (backup stream) miktarı (bayt).
    pub yedeklenmis: u64,
    /// Dosya sistemi küme boyutu (bayt). `0` geçersizdir ve `4096` sayılır.
    pub kume_bayt: u64,
}

impl Default for AyirmaGirdi {
    fn default() -> Self {
        Self {
            kapasite: 0,
            bos: 0,
            yedeklenmis: 0,
            kume_bayt: 4096,
        }
    }
}

/// Hesaplanmış ayırma denetimi sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AyirmaRaporu {
    /// Girdi kapasitesi.
    pub kapasite: u64,
    /// Taranan dosyaların toplam mantıksal boyutu.
    pub dosya_verisi: u64,
    /// Dosya başına son kısmi kümede boşa kalan alan.
    pub kume_isaret: u64,
    /// `dosya_verisi + kume_isaret`.
    pub tahmini_kullanim: u64,
    /// Girdideki yedeklenmiş küre miktarı.
    pub yedeklenmis: u64,
    /// Girdideki boş alan.
    pub bos: u64,
    /// `kapasite - (tahmini_kullanim + yedeklenmis + bos)`.
    pub kayip: i64,
    /// Modelin dayandığı varsayımları anlatan açıklama.
    pub gerekce: String,
}

impl AyirmaRaporu {
    /// JSON çıktısına çevirir.
    #[must_use]
    pub fn jsonla(&self) -> AyirmaJson {
        AyirmaJson {
            kapasite: self.kapasite,
            dosya_verisi: self.dosya_verisi,
            kume_isaret: self.kume_isaret,
            tahmini_kullanim: self.tahmini_kullanim,
            yedeklenmis: self.yedeklenmis,
            bos: self.bos,
            kayip: self.kayip,
            gerekce: self.gerekce.clone(),
        }
    }

    /// `kayip` negatifse, ölçüm eksikliğini anlatan uyarı satırı üretir.
    #[must_use]
    pub fn uyari(&self) -> Option<String> {
        if self.kayip < 0 {
            Some(format!(
                "kayip {} bayt olarak hesaplandi (negatif); bu, veri kaybi degil, \
                 modelin eksik oldugunun gostergesidir. NTFS denetim bilgisi okunmadigi icin \
                 yedeklenmis kume ve bos alan degerleri disaridan verilmelidir.",
                self.kayip
            ))
        } else {
            None
        }
    }
}

/// Ağacı ve girdiyi kullanarak ayırma denetimini hesaplar.
///
/// # Hatalar
///
/// Küme boyutu `0` ise [`Hata::GirdiGecersiz`] döner.
pub fn hesapla(agac: &Agac, girdi: AyirmaGirdi) -> Result<AyirmaRaporu, crate::hata::Hata> {
    if girdi.kume_bayt == 0 {
        return Err(crate::hata::Hata::GirdiGecersiz {
            ayrinti: "kume bayti 0 olamaz".to_string(),
        });
    }
    let (dosya_verisi, dosya_sayisi) =
        agac.on_sira()
            .into_iter()
            .fold((0_u64, 0_u64), |(bayt, adet), id| {
                let Ok(k) = agac.kayit(id) else {
                    return (bayt, adet);
                };
                if k.tur != Tur::Dosya {
                    return (bayt, adet);
                }
                (bayt.saturating_add(k.boyut), adet.saturating_add(1))
            });

    // Küme isareti: her dosya icin bos kalan son kume parcasi.
    let kume_isaret = agac.on_sira().into_iter().fold(0_u64, |t, id| {
        let Ok(k) = agac.kayit(id) else { return t };
        if k.tur != Tur::Dosya || k.boyut == 0 {
            return t;
        }
        let kume_sayisi = k.boyut.div_ceil(girdi.kume_bayt);
        let ayrilmis = kume_sayisi.saturating_mul(girdi.kume_bayt);
        t.saturating_add(ayrilmis.saturating_sub(k.boyut))
    });

    let tahmini_kullanim = dosya_verisi.saturating_add(kume_isaret);
    let bilinen = (tahmini_kullanim as i128) + (girdi.yedeklenmis as i128) + (girdi.bos as i128);
    let kayip = (girdi.kapasite as i128)
        .saturating_sub(bilinen)
        .clamp(i64::MIN as i128, i64::MAX as i128) as i64;

    let gerekce = format!(
        "dosya_verisi = {} dosyanin mantiksal bayt toplami ({}) ; kume_isaret = {} bayt \
         (kume {} bayt varsayimiyla her dosyanin bos kalan son kume parcasi) ; \
         tahmini_kullanim = dosya_verisi + kume_isaret ; yedeklenmis ve bos degerleri \
         NTFS denetim bilgisi okunmadigi icin disaridan verilmistir ; kayip = kapasite - \
         (tahmini_kullanim + yedeklenmis + bos). Bu bir tahmindir, olcum degildir: MFT, \
         $Bitmap ve muf/start alanlari okunmamistir, yalnizca dizin yuruyusu kullanilmistir.",
        dosya_sayisi, dosya_verisi, kume_isaret, girdi.kume_bayt
    );

    Ok(AyirmaRaporu {
        kapasite: girdi.kapasite,
        dosya_verisi,
        kume_isaret,
        tahmini_kullanim,
        yedeklenmis: girdi.yedeklenmis,
        bos: girdi.bos,
        kayip,
        gerekce,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hata::Hata;
    use crate::model::DuzumGirdi;

    fn agac() -> Agac {
        let mut a = Agac::yeni("/kok").expect("agac");
        // 4096 ve 5000 baytlik iki dosya: kume isareti 0 + 3192
        a.ekle(a.kok(), "/kok/a.bin", DuzumGirdi::dosya(4_096, 0))
            .expect("a");
        a.ekle(a.kok(), "/kok/b.bin", DuzumGirdi::dosya(5_000, 0))
            .expect("b");
        a.ozeti_yeniden_hesapla();
        a
    }

    fn girdi() -> AyirmaGirdi {
        AyirmaGirdi {
            kapasite: 1_000_000,
            bos: 100_000,
            yedeklenmis: 0,
            kume_bayt: 4_096,
        }
    }

    #[test]
    fn kume_isareti_kismi_kumeleri_toplar() {
        let r = hesapla(&agac(), girdi()).expect("hesap");
        assert_eq!(r.dosya_verisi, 9_096);
        assert_eq!(r.kume_isaret, 3_192);
        assert_eq!(r.tahmini_kullanim, 12_288);
    }

    #[test]
    fn kayip_kapasite_eksi_bilinen_terimlerdir() {
        let r = hesapla(&agac(), girdi()).expect("hesap");
        assert_eq!(r.kayip, 1_000_000 - (12_288 + 100_000));
        assert_eq!(r.kayip, 887_712);
    }

    #[test]
    fn yedeklenmis_kume_kaybi_azaltir() {
        let mut g = girdi();
        g.yedeklenmis = 200_000;
        let r = hesapla(&agac(), g).expect("hesap");
        assert_eq!(r.kayip, 687_712);
    }

    #[test]
    fn negatif_kayip_uyari_uretir() {
        let mut g = girdi();
        g.kapasite = 1_000;
        let r = hesapla(&agac(), g).expect("hesap");
        assert!(r.kayip < 0);
        let u = r.uyari().expect("uyari olmali");
        assert!(u.contains("negatif"));
    }

    #[test]
    fn pozitif_kayip_uyari_uretmez() {
        let r = hesapla(&agac(), girdi()).expect("hesap");
        assert!(r.uyari().is_none());
    }

    #[test]
    fn sifir_kume_boyutu_girdi_hatasi_dondurur() {
        let mut g = girdi();
        g.kume_bayt = 0;
        assert!(matches!(
            hesapla(&agac(), g),
            Err(Hata::GirdiGecersiz { .. })
        ));
    }

    #[test]
    fn bos_agac_sifir_veri_uretir() {
        let a = Agac::yeni("/kok").expect("agac");
        let r = hesapla(&a, girdi()).expect("hesap");
        assert_eq!(r.dosya_verisi, 0);
        assert_eq!(r.kume_isaret, 0);
        assert_eq!(r.kayip, 900_000);
    }

    #[test]
    fn gerekce_meti_her_terimi_adlandirir() {
        let r = hesapla(&agac(), girdi()).expect("hesap");
        assert!(r.gerekce.contains("dosya_verisi"));
        assert!(r.gerekce.contains("kume_isaret"));
        assert!(r.gerekce.contains("kayip"));
        assert!(r.gerekce.contains("MFT"));
    }

    #[test]
    fn json_donusturme_alanlari_korur() {
        let r = hesapla(&agac(), girdi()).expect("hesap");
        let j = r.jsonla();
        assert_eq!(j.kapasite, r.kapasite);
        assert_eq!(j.kayip, r.kayip);
        assert_eq!(j.gerekce, r.gerekce);
    }

    #[test]
    fn sifir_boyutlu_dosya_kume_isaretine_katilmaz() {
        let mut a = Agac::yeni("/kok").expect("agac");
        a.ekle(a.kok(), "/kok/bos.bin", DuzumGirdi::dosya(0, 0))
            .expect("bos");
        a.ozeti_yeniden_hesapla();
        let r = hesapla(&a, girdi()).expect("hesap");
        assert_eq!(r.kume_isaret, 0);
        assert_eq!(r.dosya_verisi, 0);
    }

    #[test]
    fn varsayilan_girdi_gecerli_bir_kume_boyutu_icerir() {
        assert_eq!(AyirmaGirdi::default().kume_bayt, 4096);
    }
}

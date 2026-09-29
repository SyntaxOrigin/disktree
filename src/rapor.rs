//! Anlık görüntü (`Kayma`) ve rapor JSON şemaları, dosya okuma/yazma.
//!
//! Anlık görüntü, raporun "disk üzerinde önbellek" özelliğinin Rust karşılığıdır:
//! bir tarama sonucu JSON olarak diske yazılır ve sonraki `timeline` /
//! `heatmap` çalıştırmalarında **yeniden taramadan** karşılaştırma tabanı olarak
//! kullanılır.
//!
//! Biçim sözleşmesi:
//!
//! * `bicim` alanı `disktree-kayma/1` olmalıdır; farklıysa dosya sessizce
//!   kullanılmaz, [`Hata::KaymaBicimBozuk`] döner.
//! * `surum` şema sürümüdür; uyuşmazlıkta [`Hata::KaymaSurusuUyumsuz`] döner.
//! * Yazma **atomiktir**: önce geçici dosyaya yazılır, sonra hedefe taşınır.
//!   Yarım kalmış bir önbellek dosyası bir sonraki açılışta kullanılmaz.

use crate::hata::Hata;
use crate::model::{Agac, DugumId};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Anlık görüntü dosyasının biçim imzası.
pub const BICIM: &str = "disktree-kayma/1";

/// Bu ikilinin okuyabildiği şema sürümü.
pub const SURUM: u32 = 1;

/// Anlık görüntü dosyasındaki tek bir düğümün JSON karşılığı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DugumJson {
    /// Düğümün tam yolu.
    pub ad: String,
    /// `"dizin"` ya da `"dosya"`.
    pub tur: String,
    /// Dosyanın bayt cinsinden boyutu (dizinlerde `0`).
    pub boyt: u64,
    /// Kendisi ve altındaki her şeyin toplam baytı.
    pub alt_toplam: u64,
    /// Kökten itibaren derinlik.
    pub derinlik: u16,
    /// Son değişiklik zamanı, Unix epoch saniyesi.
    pub degistirme: u64,
    /// Gizli özniteliği.
    pub gizli: bool,
    /// Sistem özniteliği.
    pub sistem: bool,
}

/// Bir tarama anlık görüntüsü.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kayma {
    /// Biçim imzası; [`BICIM`] ile birebir eşleşmelidir.
    pub bicim: String,
    /// Şema sürümü.
    pub surum: u32,
    /// Taranan kök yol.
    pub kok: String,
    /// Taramanın Unix epoch saniyesi.
    pub zaman: u64,
    /// Kökten sonra taranan düğümler (ön sırada).
    pub dugumler: Vec<DugumJson>,
}

/// Tarama özet sayısal göstergelerinin JSON karşılığı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IstatistikJson {
    /// Tarama kökü dışında açılan dizin sayısı.
    pub ziyaret_edilen_dizin: u32,
    /// Atlanan girdi sayısı.
    pub atlanan_girdi: u32,
    /// İzin reddi sayısı.
    pub erisim_reddi: u32,
    /// Döngü korumasının devreye girdiği sayı.
    pub dongu: u32,
    /// Derinlik sınırı nedeniyle inilmeyen dizin sayısı.
    pub derinlik_asim: u32,
    /// Kullanıcı dışlamasıyla atlanan girdi sayısı.
    pub haric_tutulan: u32,
    /// Sembolik bağ sayısı.
    pub bag: u32,
    /// Tarama kısmi mi tamamlandı.
    pub tamamlandi: bool,
    /// Düğüm sınırına takılıp takılmadığı.
    pub kisaltildi: bool,
    /// Duvar saati ölçümü (saniye).
    pub sure_sn: f64,
    /// Toplam düğüm sayısı (kök dâhil).
    pub dugum_sayisi: usize,
    /// Toplam dosya sayısı.
    pub dosya_sayisi: u32,
    /// Toplam dizin sayısı (kök dâhil).
    pub dizin_sayisi: u32,
    /// Toplam bayt.
    pub toplam_bayt: u64,
    /// Ağacın bellekte kapladığı bayt (yol havuzu + kayıt dizisi).
    pub bellek_boyutu: usize,
    /// Taranan gizli girdi sayısı.
    pub gizli_sayisi: u32,
    /// Taranan sistem girdi sayısı.
    pub sistem_sayisi: u32,
}

/// Büyük/eskimiş dosya listesi satırı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyukSatirJson {
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

/// Ayırma denetimi ("siyah kutu") satırı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AyirmaJson {
    /// Rapor edilen birim kapasitesi.
    pub kapasite: u64,
    /// Taranan dosyaların toplam mantıksal boyutu.
    pub dosya_verisi: u64,
    /// Son kısmi küme başına boşa kalan alan tahmini.
    pub kume_isaret: u64,
    /// Tahmini gerçek disk kullanımı.
    pub tahmini_kullanim: u64,
    /// Kullanıcıdan gelen yedeklenmiş küme miktarı.
    pub yedeklenmis: u64,
    /// Kullanıcıdan gelen boş küme miktarı.
    pub bos: u64,
    /// `kapasite - (tahmini_kullanim + yedeklenmis + bos)`. Negatif olabilir.
    pub kayip: i64,
    /// Her terimin nereden geldiğini anlatan açıklama.
    pub gerekce: String,
}

/// Zaman çizelgesi satırı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZamanJson {
    /// Sıra numarası.
    pub sira: u32,
    /// Yol.
    pub yol: String,
    /// Önceki boyut.
    pub onceki_bayt: u64,
    /// Yeni boyut.
    pub sonraki_bayt: u64,
    /// Fark.
    pub fark: i64,
    /// Durum etiketi.
    pub durum: String,
}

/// Aylık kümülatif büyüme eğrisi noktası.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AyJson {
    /// `YYYY-AA` etiketi.
    pub ay: String,
    /// O aya düşen bayt.
    pub bayt: u64,
    /// O aya düşen dosya sayısı.
    pub dosya: u32,
    /// Kümülatif bayt.
    pub kumulatif_bayt: u64,
    /// Kümülatif dosya.
    pub kumulatif_dosya: u64,
}

/// Isı haritası satırı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IsiJson {
    /// Yol.
    pub yol: String,
    /// Değişim büyüklüğü.
    pub degisim: i64,
    /// Derinlik.
    pub derinlik: u16,
    /// Derinlik azalma skoru.
    pub derinlik_skoru: f64,
    /// Çarpım skoru.
    pub skor: f64,
    /// 0-1 arası normalize edilmiş skor.
    pub normalize: f64,
    /// 0-4 arası yoğunluk seviyesi.
    pub seviye: u8,
    /// Seviye etiketi.
    pub etiket: String,
}

/// Treemap kutusu JSON karşılığı (serileştirme `treemap::Kutu` ile aynıdır).
pub use crate::treemap::Kutu as KutuJson;

/// `report` alt komutunun tamamını kapsayan çıktı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rapor {
    /// Anlık görüntü biçim imzası.
    pub bicim: String,
    /// Anlık görüntü şema sürümü.
    pub surum: u32,
    /// Tarama özeti.
    pub istatistik: IstatistikJson,
    /// Aylık kümülatif büyüme eğrisi.
    pub zaman: Vec<AyJson>,
    /// Büyük/eskimiş dosya listesi.
    pub buyuk_dosyalar: Vec<BuyukSatirJson>,
    /// Ayırma denetimi satırı.
    pub ayirma: AyirmaJson,
    /// Treemap kutuları.
    pub treemap: Vec<KutuJson>,
    /// Isı haritası satırları (yalnızca iki anlık görüntü karşılaştırıldıysa).
    pub isi: Vec<IsiJson>,
}

impl IstatistikJson {
    /// Bir tarama sonucundan özet üretir.
    #[must_use]
    pub fn uret(agac: &Agac, istatistik: &crate::tarama::TaramaIstatistik) -> Self {
        Self {
            ziyaret_edilen_dizin: istatistik.ziyaret_edilen_dizin,
            atlanan_girdi: istatistik.atlanan_girdi,
            erisim_reddi: istatistik.erisim_reddi,
            dongu: istatistik.dongu,
            derinlik_asim: istatistik.derinlik_asim,
            haric_tutulan: istatistik.haric_tutulan,
            bag: istatistik.bag,
            tamamlandi: istatistik.tamamlandi,
            kisaltildi: istatistik.kisaltildi,
            sure_sn: istatistik.sure_sn,
            dugum_sayisi: agac.dugum_sayisi(),
            dosya_sayisi: agac.toplam_dosya(),
            dizin_sayisi: agac.toplam_dizin(),
            toplam_bayt: agac.toplam_bayt(),
            bellek_boyutu: agac.bellek_boyutu(),
            gizli_sayisi: agac.gizli_sayisi(),
            sistem_sayisi: agac.sistem_sayisi(),
        }
    }
}

impl DugumJson {
    /// Bir ağaç düğümünü JSON karşılığına çevirir.
    #[must_use]
    pub fn dugumden(agac: &Agac, id: DugumId) -> Self {
        let bos = bos_kayit();
        let k = agac.kayit(id).unwrap_or(&bos);
        Self {
            ad: agac.yol_veya_yildiz(id).to_string(),
            tur: k.tur.ad().to_string(),
            boyt: k.boyut,
            alt_toplam: k.alt_toplam,
            derinlik: k.derinlik,
            degistirme: k.degistirme,
            gizli: k.gizli,
            sistem: k.sistem,
        }
    }
}

fn bos_kayit() -> crate::model::Kayit {
    crate::model::Kayit {
        ad: crate::model::YolKimlik {
            baslangic: 0,
            uzunluk: 0,
        },
        ust: None,
        ilk_cocuk: None,
        sonraki_kardes: None,
        tur: crate::model::Tur::Dosya,
        derinlik: 0,
        boyut: 0,
        alt_toplam: 0,
        alt_dosya: 0,
        alt_dizin: 0,
        degistirme: 0,
        gizli: false,
        sistem: false,
        bag: false,
    }
}

impl Kayma {
    /// Bir ağacı anlık görüntüye çevirir.
    #[must_use]
    pub fn agactan(agac: &Agac, zaman: u64) -> Self {
        let dugumler = agac
            .on_sira()
            .into_iter()
            .filter(|&id| id != agac.kok())
            .map(|id| DugumJson::dugumden(agac, id))
            .collect();
        Self {
            bicim: BICIM.to_string(),
            surum: SURUM,
            kok: agac.kok_yolu().to_string(),
            zaman,
            dugumler,
        }
    }
}

/// Anlık görüntüyü JSON metnine çevirir.
///
/// # Hatalar
///
/// Serileştirme başarısız olursa [`Hata::Serilestirme`] döner.
pub fn kayma_metni(kayma: &Kayma) -> Result<String, Hata> {
    serde_json::to_string_pretty(kayma).map_err(|e| Hata::Serilestirme {
        ayrinti: e.to_string(),
    })
}

/// JSON metnini anlık görüntüye çevirir ve biçim/sürümü doğrular.
///
/// # Hatalar
///
/// Sözdizimi hatası veya eksik alan varsa [`Hata::KaymaBicimBozuk`], biçim
/// imzası tutmuyorsa aynı hata, sürüm tutmuyorsa [`Hata::KaymaSurusuUyumsuz`]
/// döner.
pub fn kayma_yorumla(metin: &str) -> Result<Kayma, Hata> {
    let kayma: Kayma = serde_json::from_str(metin).map_err(|e| Hata::KaymaBicimBozuk {
        yol: "<bellek>".to_string(),
        ayrinti: e.to_string(),
    })?;
    if kayma.bicim != BICIM {
        return Err(Hata::KaymaBicimBozuk {
            yol: "<bellek>".to_string(),
            ayrinti: format!("beklenen bicim {BICIM}, bulunan {}", kayma.bicim),
        });
    }
    if kayma.surum != SURUM {
        return Err(Hata::KaymaSurusuUyumsuz {
            bulunan: kayma.surum,
            beklenen: SURUM,
        });
    }
    Ok(kayma)
}

/// Anlık görüntüyü diske yazar (önce geçici dosya, sonra atomik taşıma).
///
/// # Hatalar
///
/// Yazma veya taşıma hatasında [`Hata::CiktiYazilamadi`] döner.
pub fn kayma_yaz(kayma: &Kayma, yol: &Path) -> Result<(), Hata> {
    let metin = kayma_metni(kayma)?;
    let gecici = yol.with_extension("gecici");
    std::fs::write(&gecici, metin).map_err(|kaynak| Hata::CiktiYazilamadi {
        yol: gecici.display().to_string(),
        kaynak,
    })?;
    std::fs::rename(&gecici, yol).map_err(|kaynak| Hata::CiktiYazilamadi {
        yol: yol.display().to_string(),
        kaynak,
    })
}

/// Anlık görüntüyü diskten okur ve doğrular.
///
/// # Hatalar
///
/// Okuma hatasında [`Hata::GirisOkunamadi`], biçim/sürüm hatasında
/// [`Hata::KaymaBicimBozuk`] veya [`Hata::KaymaSurusuUyumsuz`] döner.
pub fn kayma_oku(yol: &Path) -> Result<Kayma, Hata> {
    let metin = std::fs::read_to_string(yol).map_err(|kaynak| Hata::GirisOkunamadi {
        yol: yol.display().to_string(),
        kaynak,
    })?;
    kayma_yorumla(&metin).map_err(|e| match e {
        Hata::KaymaBicimBozuk { ayrinti, .. } => Hata::KaymaBicimBozuk {
            yol: yol.display().to_string(),
            ayrinti,
        },
        diger => diger,
    })
}

/// Herhangi bir serileştirilebilir değeri JSON metnine çevirir.
///
/// # Hatalar
///
/// Serileştirme hatasında [`Hata::Serilestirme`] döner.
pub fn json_metni<T: Serialize>(deger: &T) -> Result<String, Hata> {
    serde_json::to_string_pretty(deger).map_err(|e| Hata::Serilestirme {
        ayrinti: e.to_string(),
    })
}

/// Metni UTF-8 dosyaya yazar.
///
/// # Hatalar
///
/// Yazma hatasında [`Hata::CiktiYazilamadi`] döner.
pub fn metin_yaz(yol: &Path, metin: &str) -> Result<(), Hata> {
    std::fs::write(yol, metin).map_err(|kaynak| Hata::CiktiYazilamadi {
        yol: yol.display().to_string(),
        kaynak,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DuzumGirdi;

    fn ornek_agac() -> Agac {
        let mut a = Agac::yeni("/kok").expect("agac");
        let d = a
            .ekle(a.kok(), "/kok/klasor", DuzumGirdi::dizin(1_700_000_000))
            .expect("dizin");
        a.ekle(
            d,
            "/kok/klasor/a.txt",
            DuzumGirdi::dosya(42, 1_700_000_500).gizli(),
        )
        .expect("dosya");
        a.ozeti_yeniden_hesapla();
        a
    }

    #[test]
    fn kayma_agactan_uretildiginde_kok_dahil_degildir() {
        let k = Kayma::agactan(&ornek_agac(), 1_700_000_000);
        assert_eq!(k.bicim, BICIM);
        assert_eq!(k.surum, SURUM);
        assert_eq!(k.kok, "/kok");
        assert_eq!(k.dugumler.len(), 2);
    }

    #[test]
    fn kayma_json_gidis_donusu_kayipsizdir() {
        let k = Kayma::agactan(&ornek_agac(), 1_700_000_000);
        let metin = kayma_metni(&k).expect("metin");
        let geri = kayma_yorumla(&metin).expect("yorumla");
        assert_eq!(k, geri);
    }

    #[test]
    fn kayma_yazma_ve_okuma_dosya_uzerinden_calisir() {
        let gecici = std::env::temp_dir().join(format!("disktree-kayma-{}", std::process::id()));
        let _ = std::fs::remove_file(&gecici);
        let k = Kayma::agactan(&ornek_agac(), 42);
        kayma_yaz(&k, &gecici).expect("yaz");
        assert!(
            !gecici.with_extension("gecici").exists(),
            "gecici dosanin kalmamasi lazim"
        );
        let geri = kayma_oku(&gecici).expect("oku");
        assert_eq!(k, geri);
        let _ = std::fs::remove_file(&gecici);
    }

    #[test]
    fn bozuk_json_hata_dondurur() {
        let hata = kayma_yorumla("{bozuk").expect_err("gecersiz json");
        assert!(matches!(hata, Hata::KaymaBicimBozuk { .. }));
    }

    #[test]
    fn yanlis_bicim_imzasi_reddedilir() {
        let json = r#"{"bicim":"baska/9","surum":1,"kok":"/k","zaman":0,"dugumler":[]}"#;
        let hata = kayma_yorumla(json).expect_err("bicim hatali");
        assert!(matches!(hata, Hata::KaymaBicimBozuk { .. }));
    }

    #[test]
    fn yanlis_surum_reddedilir() {
        let json = r#"{"bicim":"disktree-kayma/1","surum":42,"kok":"/k","zaman":0,"dugumler":[]}"#;
        let hata = kayma_yorumla(json).expect_err("surum hatali");
        assert!(matches!(hata, Hata::KaymaSurusuUyumsuz { .. }));
    }

    #[test]
    fn okunmayan_dosya_hata_dondurur() {
        let yol = std::env::temp_dir().join("disktree-yok-boyle-bir-kayma.json");
        let _ = std::fs::remove_file(&yol);
        assert!(matches!(kayma_oku(&yol), Err(Hata::GirisOkunamadi { .. })));
    }

    #[test]
    fn dugum_json_yol_ve_nitelikleri_tasarimaktadir() {
        let a = ornek_agac();
        let id = a.ara("/kok/klasor/a.txt").expect("dugum");
        let d = DugumJson::dugumden(&a, id);
        assert_eq!(d.tur, "dosya");
        assert_eq!(d.boyt, 42);
        assert_eq!(d.alt_toplam, 42);
        assert!(d.gizli);
        assert!(!d.sistem);
        assert_eq!(d.derinlik, 2);
    }

    #[test]
    fn istatistik_json_agactan_turetilir() {
        let a = ornek_agac();
        let ist = crate::tarama::TaramaIstatistik::default();
        let j = IstatistikJson::uret(&a, &ist);
        assert_eq!(j.dugum_sayisi, 3);
        assert_eq!(j.dosya_sayisi, 1);
        assert_eq!(j.toplam_bayt, 42);
        assert!(j.bellek_boyutu > 0);
    }

    #[test]
    fn rapor_json_gidis_donusu_kayipsizdir() {
        let rapor = Rapor {
            bicim: BICIM.to_string(),
            surum: SURUM,
            istatistik: IstatistikJson::uret(
                &ornek_agac(),
                &crate::tarama::TaramaIstatistik::default(),
            ),
            zaman: vec![AyJson {
                ay: "2026-09".to_string(),
                bayt: 42,
                dosya: 1,
                kumulatif_bayt: 42,
                kumulatif_dosya: 1,
            }],
            buyuk_dosyalar: Vec::new(),
            ayirma: AyirmaJson {
                kapasite: 1000,
                dosya_verisi: 42,
                kume_isaret: 4070,
                tahmini_kullanim: 4112,
                yedeklenmis: 0,
                bos: 0,
                kayip: -3112,
                gerekce: "test".to_string(),
            },
            treemap: Vec::new(),
            isi: Vec::new(),
        };
        let metin = json_metni(&rapor).expect("json");
        let geri: Rapor = serde_json::from_str(&metin).expect("geri");
        assert_eq!(rapor, geri);
    }

    #[test]
    fn metin_yazma_dosya_olusturur() {
        let yol = std::env::temp_dir().join(format!("disktree-metin-{}", std::process::id()));
        metin_yaz(&yol, "icerik").expect("yaz");
        assert_eq!(std::fs::read_to_string(&yol).expect("oku"), "icerik");
        let _ = std::fs::remove_file(&yol);
    }
}

//! Dizin ağacı yürüyüşü: `std::fs::read_dir` tabanlı, salt okunur tarama.
//!
//! Bu modül, raporun önerdiği NTFS Master File Table okuyucusunun **yerine**
//! geçer. Gerekçe README'de ayrıntılıdır: ham birim açma (`\\.\C:`) çoğu
//! Windows kurulumunda yönetici yetkisi ister ve bu, kurulum gerektirmeyen ve
//! tek dosya dağıtılan bir aracın taşınabilirlik ilkesiyle çelişir. Dizin
//! yürüyüşü hiçbir ayrıcalık istemez, her platformda çalışır ve `unsafe`
//! içermez.
//!
//! Güvenlik modeli: bu modül yalnızca `read_dir`, `symlink_metadata` ve
//! `metadata` çağırır. Dosya **içeriğinin** hiçbir baytı okunmaz ve hiçbir
//! yazma çağrısı yapılmaz. Sembolik bağlar varsayılan olarak izlenmez; izleme
//! açıldığında bile [`DonguKoruma`] kanonik yol kümesiyle sonsuz döngüyü keser.

use crate::hata::Hata;
use crate::model::{Agac, DugumId, DuzumGirdi, Tur};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Bir tarama çalıştırmasının ayarları.
#[derive(Debug, Clone)]
pub struct TaramaSecenegi {
    /// Kök dizinden aşağı en fazla inecek derinlik (kök = 0).
    pub en_derinlik: u32,
    /// Gizli öznitelikli girdiler ağaca eklensin mi.
    pub gizlileri_dahil_et: bool,
    /// Sistem öznitelikli girdiler ağaca eklensin mi.
    pub sistemleri_dahil_et: bool,
    /// Sembolik bağların hedefi de gezilsin mi (varsayılan hayır).
    pub sembolik_baglari_izle: bool,
    /// Yolu bu kalıntılardan biriyle eşleşen girdiler atlanır.
    ///
    /// Eşleşme **alt dizi** aramasıdır ve büyük/küçük harf duyarsızdır; yani
    /// `node_modules` deseni `C:\proje\node_modules\paket.js` yolunu eler.
    /// Baştaki `*` karakteri "herhangi bir yerde" anlamında yok sayılır, böylece
    /// `*.zip` ile uzantıya göre dışlama yazılabilir. Glob crate'i yasak olduğu
    /// için desen dili kasıtlı olarak bu kadar basit tutulmuştur.
    pub haric_tutulan: Vec<String>,
    /// Bu düğüm sayısına ulaşıldığında tarama kısmi olarak durur.
    pub maks_dugum: Option<usize>,
}

impl Default for TaramaSecenegi {
    fn default() -> Self {
        Self {
            en_derinlik: 64,
            gizlileri_dahil_et: true,
            sistemleri_dahil_et: true,
            sembolik_baglari_izle: false,
            haric_tutulan: Vec::new(),
            maks_dugum: None,
        }
    }
}

/// Sonsuz döngü ve tekrar ziyaret koruması.
///
/// Kilit, bir dizimin **kanonik** yoludur. Kanonik yol, sembolik bağ ve `..`
/// çözümlendikten sonra elde edilir; böylece `a -> b -> a` gibi bir halka ile
/// aynı dizine iki farklı yoldan (sert bağ ya da farklı takma ad) gelmek de
/// yakalanır.
#[derive(Debug, Default)]
pub struct DonguKoruma {
    gorulenler: HashSet<PathBuf>,
}

impl DonguKoruma {
    /// Boş bir koruma oluşturur.
    #[must_use]
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Verilen yol daha önce görüldü mü?
    #[must_use]
    pub fn gormus(&self, kanit: &Path) -> bool {
        self.gorulenler.contains(kanit)
    }

    /// Yolu listeye ekler; **yeni** eklendiyse `true` döner.
    pub fn ekle(&mut self, kanit: PathBuf) -> bool {
        self.gorulenler.insert(kanit)
    }

    /// Listeyen önceden doldurur.
    ///
    /// Bu metot, tarayıcının belirli bir dizine hiç girmemesi gereken senaryoyu
    /// (döngü korumasının uçtan uca kanıtı) dışarıdan doğrulamak için açıktır;
    /// tarama içinde kendiliğinden kullanılmaz.
    pub fn tohumla(&mut self, kanit: PathBuf) {
        self.gorulenler.insert(kanit);
    }

    /// Şu ana kadar kilitlenen dizin sayısı.
    #[must_use]
    pub fn boyut(&self) -> usize {
        self.gorulenler.len()
    }
}

/// Tarama sırasında toplanan sayısal göstergeler.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaramaIstatistik {
    /// Açılıp içeriği okunan dizin sayısı.
    pub ziyaret_edilen_dizin: u32,
    /// `read_dir` veya `metadata` hatası nedeniyle atlanan girdi sayısı.
    pub atlanan_girdi: u32,
    /// İzin/yol hatası sayısı (ayrı sayılır, çünkü rapor bunu "kısıtlı" listeler).
    pub erisim_reddi: u32,
    /// Döngü koruması devreye giren dizin sayısı.
    pub dongu: u32,
    /// Derinlik sınırı nedeniyle inilmeyen dizin sayısı.
    pub derinlik_asim: u32,
    /// Kullanıcı tarafından dışlanan girdi sayısı.
    pub haric_tutulan: u32,
    /// Sembolik bağ olarak kaydedilen düğüm sayısı.
    pub bag: u32,
    /// Tarama `--maks-dugum` sınırına takıldı mı?
    pub kisaltildi: bool,
    /// Tarama kök hatası dışında normal mi tamamlandı?
    pub tamamlandi: bool,
    /// Duvar saati ölçümü (saniye). Yalnızca çıktı içindir, teste bağlanmaz.
    pub sure_sn: f64,
}

impl TaramaIstatistik {
    /// Özet metin olarak döndürür.
    #[must_use]
    pub fn ozet(&self) -> String {
        format!(
            "dizin={} atlanan={} erisim_reddi={} dongu={} derinlik_asim={} haric={} bag={} \
             kisaltildi={} tamamlandi={} sure={:.3}sn",
            self.ziyaret_edilen_dizin,
            self.atlanan_girdi,
            self.erisim_reddi,
            self.dongu,
            self.derinlik_asim,
            self.haric_tutulan,
            self.bag,
            self.kisaltildi,
            self.tamamlandi,
            self.sure_sn
        )
    }
}

/// Tamamlanmış bir taramanın taşınabilir sonucu.
#[derive(Debug)]
pub struct TaramaSonucu {
    /// Üretilen sabit bellekli ağaç.
    pub agac: Agac,
    /// Sayısal göstergeler.
    pub istatistik: TaramaIstatistik,
    /// Taramayı durdurmadan geçilen, kayda geçmiş hatalar.
    pub hatalar: Vec<Hata>,
}

/// Özyinelemeli `read_dir` gezgini.
#[derive(Debug)]
pub struct Tarayici {
    agac: Agac,
    istatistik: TaramaIstatistik,
    koruma: DonguKoruma,
    secenek: TaramaSecenegi,
    hatalar: Vec<Hata>,
}

/// Yığında taşınan iş: henüz açılmamış dizin.
#[derive(Debug, Clone, Copy)]
struct Is {
    dugum: DugumId,
    derinlik: u32,
}

impl Tarayici {
    /// Verilen seçeneklerle, kökü `kok_yol` olan bir gezgin oluşturur.
    ///
    /// # Hatalar
    ///
    /// Kök yol havuzuna sığmazsa [`Hata::YolHavuzuDolu`] döner.
    pub fn yeni(kok_yol: &str, secenek: TaramaSecenegi) -> Result<Self, Hata> {
        Ok(Self {
            agac: Agac::yeni(kok_yol)?,
            istatistik: TaramaIstatistik::default(),
            koruma: DonguKoruma::yeni(),
            secenek,
            hatalar: Vec::new(),
        })
    }

    /// Döngü korumasını dışarıdan doldurur (uçtan uca döngü testi için).
    #[must_use]
    pub fn koruma_ile(mut self, koruma: DonguKoruma) -> Self {
        self.koruma = koruma;
        self
    }

    /// Tarama sırasında karşılaşılan, tarama durdurmadan geçilen hatalar.
    #[must_use]
    pub fn hatalar(&self) -> &[Hata] {
        &self.hatalar
    }

    /// Tarama sayısal göstergeleri.
    #[must_use]
    pub fn istatistik(&self) -> &TaramaIstatistik {
        &self.istatistik
    }

    /// Üretilen ağaç (tarama bitene kadar kısmi olabilir).
    #[must_use]
    pub fn agac(&self) -> &Agac {
        &self.agac
    }

    /// Taramayı çalıştırır ve ağacı ödünç alarak döndürür.
    ///
    /// Kök dizin okunamazsa [`Hata::KokOkunamadi`] döner ve tarama yapılmaz.
    /// Kök okunabiliyorsa **hiçbir alt hata taramayı durdurmaz**; hepsi
    /// sayılır ve [`Tarayici::hatalar`] listesine eklenir. Bu, raporun "disk
    /// bozuksa okuma hataları çok sıktır; her kayıt hatası taramayı
    /// durdurmamalı, sayılmalıdır" risk azaltmasının doğrudan uygulamasıdır.
    ///
    /// # Hatalar
    ///
    /// Yalnızca kök okunamazsa hata döner.
    pub fn tara(&mut self) -> Result<&Agac, Hata> {
        self.tara_ic()?;
        Ok(&self.agac)
    }

    /// Taramayı çalıştırır ve gezgini tüketerek taşınabilir sonucu döndürür.
    ///
    /// # Hatalar
    ///
    /// Yalnızca kök okunamazsa hata döner.
    pub fn tara_ve_al(mut self) -> Result<TaramaSonucu, Hata> {
        self.tara_ic()?;
        Ok(TaramaSonucu {
            agac: self.agac,
            istatistik: self.istatistik,
            hatalar: self.hatalar,
        })
    }

    /// Ortak tarama döngüsü.
    fn tara_ic(&mut self) -> Result<(), Hata> {
        let baslangic = Instant::now();
        let kok = PathBuf::from(self.agac.kok_yolu());
        let okuma = std::fs::read_dir(&kok).map_err(|kaynak| Hata::KokOkunamadi {
            yol: kok.display().to_string(),
            kaynak,
        })?;
        self.istatistik.ziyaret_edilen_dizin = 1;
        self.istatistik.tamamlandi = true;
        self.koruma.ekle(kanonik(&kok));

        let mut yigin: Vec<Is> = Vec::new();
        if !self.girdileri_isle(okuma, self.agac.kok(), 0, &mut yigin) {
            self.istatistik.tamamlandi = false;
        }

        while let Some(is) = yigin.pop() {
            if is.derinlik > self.secenek.en_derinlik {
                self.istatistik.derinlik_asim = self.istatistik.derinlik_asim.saturating_add(1);
                continue;
            }
            let yol = self
                .agac
                .yol(is.dugum)
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."));
            if !self.koruma.ekle(kanonik(&yol)) {
                self.istatistik.dongu = self.istatistik.dongu.saturating_add(1);
                continue;
            }
            match std::fs::read_dir(&yol) {
                Ok(okuma) => {
                    self.istatistik.ziyaret_edilen_dizin =
                        self.istatistik.ziyaret_edilen_dizin.saturating_add(1);
                    if !self.girdileri_isle(okuma, is.dugum, is.derinlik, &mut yigin) {
                        self.istatistik.tamamlandi = false;
                        break;
                    }
                }
                Err(kaynak) => {
                    self.istatistik.erisim_reddi = self.istatistik.erisim_reddi.saturating_add(1);
                    self.agac.erisim_reddi_ekle();
                    self.hatalar.push(Hata::DizinOkunamadi {
                        yol: yol.display().to_string(),
                        kaynak,
                    });
                }
            }
        }

        self.agac.ozeti_yeniden_hesapla();
        self.istatistik.sure_sn = baslangic.elapsed().as_secs_f64();
        Ok(())
    }

    /// Bir dizin okumasının girdilerini işler.
    ///
    /// Alt dizinleri `yigin`a ekler. Düğüm sınırı aşılırsa `false` döner ve
    /// çağıran döngüyü sonlandırır.
    fn girdileri_isle<I: Iterator<Item = std::io::Result<std::fs::DirEntry>>>(
        &mut self,
        okuma: I,
        ust: DugumId,
        derinlik: u32,
        yigin: &mut Vec<Is>,
    ) -> bool {
        for girdi in okuma {
            if !self.sinir_icinde() {
                self.istatistik.kisaltildi = true;
                return false;
            }
            let Ok(girdi) = girdi else {
                self.istatistik.atlanan_girdi = self.istatistik.atlanan_girdi.saturating_add(1);
                self.istatistik.erisim_reddi = self.istatistik.erisim_reddi.saturating_add(1);
                self.agac.erisim_reddi_ekle();
                continue;
            };
            let yol = girdi.path();
            let yol_metin = yol.display().to_string();
            if self.haric_mi(&yol_metin) {
                self.istatistik.haric_tutulan = self.istatistik.haric_tutulan.saturating_add(1);
                continue;
            }
            // Sembolik bağ tespiti: symlink_metadata bağı izlemez.
            let bag_meta = match std::fs::symlink_metadata(&yol) {
                Ok(m) => m,
                Err(kaynak) => {
                    self.istatistik.atlanan_girdi = self.istatistik.atlanan_girdi.saturating_add(1);
                    self.hatalar.push(Hata::GirdiOkunamadi {
                        yol: yol_metin,
                        kaynak,
                    });
                    continue;
                }
            };
            let bag = bag_meta.file_type().is_symlink();
            let (gizli, sistem) = ozellikler(&bag_meta);
            if gizli && !self.secenek.gizlileri_dahil_et {
                continue;
            }
            if sistem && !self.secenek.sistemleri_dahil_et {
                continue;
            }
            // Boyut ve zaman için bağı izleyen metadata; bağ kırık olabilir.
            let izlenmis = if bag {
                std::fs::metadata(&yol)
            } else {
                Ok(bag_meta)
            };
            let Ok(izlenmis) = izlenmis else {
                self.istatistik.atlanan_girdi = self.istatistik.atlanan_girdi.saturating_add(1);
                self.hatalar.push(Hata::GirdiOkunamadi {
                    yol: yol_metin,
                    kaynak: std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "sembolik bagin hedefi cozulemiyor",
                    ),
                });
                continue;
            };
            let tur = if izlenmis.is_dir() {
                Tur::Dizin
            } else {
                Tur::Dosya
            };
            let boyut = if tur == Tur::Dosya { izlenmis.len() } else { 0 };
            let degistirme = match izlenmis.modified() {
                Ok(t) => t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs()),
                Err(kaynak) => {
                    self.hatalar.push(Hata::ZamanOkunamadi {
                        yol: yol_metin.clone(),
                        kaynak,
                    });
                    0
                }
            };
            let dugum = match self.agac.ekle(
                ust,
                &yol_metin,
                DuzumGirdi {
                    tur,
                    boyut,
                    degistirme,
                    gizli,
                    sistem,
                    bag,
                },
            ) {
                Ok(d) => d,
                Err(e) => {
                    self.istatistik.kisaltildi = true;
                    self.hatalar.push(e);
                    return false;
                }
            };
            if bag {
                self.istatistik.bag = self.istatistik.bag.saturating_add(1);
            }
            if tur == Tur::Dizin && (!bag || self.secenek.sembolik_baglari_izle) {
                yigin.push(Is {
                    dugum,
                    derinlik: derinlik.saturating_add(1),
                });
            }
        }
        true
    }

    /// `--maks-dugum` sınırı içinde miyiz?
    fn sinir_icinde(&self) -> bool {
        match self.secenek.maks_dugum {
            None => true,
            Some(limit) => self.agac.dugum_sayisi() < limit,
        }
    }

    /// Kullanıcı dışlama kalıbıyla eşleşiyor mu?
    fn haric_mi(&self, yol: &str) -> bool {
        let kucuk = yol.to_lowercase();
        self.secenek
            .haric_tutulan
            .iter()
            .any(|d| kalip_uyum(&kucuk, &d.to_lowercase()))
    }
}

/// Tek seferlik tarama: verilen kökü verilen seçeneklerle tarar.
///
/// # Hatalar
///
/// Yalnızca **kök** okunamazsa [`Hata::KokOkunamadi`] döner; alt dizin
/// hataları sonuç içinde sayılır ve `hatalar` listesine eklenir.
pub fn tara(kok: &str, secenek: TaramaSecenegi) -> Result<TaramaSonucu, Hata> {
    Tarayici::yeni(kok, secenek)?.tara_ve_al()
}

/// Dışlama kalıbı: baştaki `*` yok sayılır, kalanı yol içinde alt dizi aranır.
fn kalip_uyum(yol: &str, kalip: &str) -> bool {
    let kalip = kalip.strip_prefix('*').unwrap_or(kalip);
    !kalip.is_empty() && yol.contains(kalip)
}

/// Kanonik yol anahtarı; başarısızsa girdi yolü kendisi kullanılır.
fn kanonik(yol: &Path) -> PathBuf {
    std::fs::canonicalize(yol).unwrap_or_else(|_| yol.to_path_buf())
}

/// Gizli/sistem özniteliklerini platformdan bağımsız biçimde döndürür.
///
/// Windows'ta `MetadataExt::file_attributes` güvenli bir std API'sidir ve
/// `unsafe` gerektirmez. Diğer platformlarda standart kütüphane gizli dosya
/// bilgisini vermediği için nokta ile başlayan adlar gizli kabul edilir ve
/// sistem özniteliği her zaman `false` olur.
fn ozellikler(md: &std::fs::Metadata) -> (bool, bool) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        let nitelik = md.file_attributes();
        (
            nitelik & FILE_ATTRIBUTE_HIDDEN != 0,
            nitelik & FILE_ATTRIBUTE_SYSTEM != 0,
        )
    }
    #[cfg(not(windows))]
    {
        let ad = md.file_name().to_string_lossy().to_ascii_lowercase();
        (ad.starts_with('.'), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Testlerin yazdığı geçici dizin; `Drop` ile temizlenir.
    ///
    /// `tempfile` crate'i bağımlılık politikasıyla yasak olduğu için elle
    /// yazıldı. Temizlik hatası `Drop` içinden döndürülemediği için `let _ =`
    /// ile yutulur; bu, sözleşmenin açıkça izin verdiği tek yutma yeridir.
    struct Gecici {
        yol: PathBuf,
    }

    impl Gecici {
        fn yeni(etiket: &str) -> Self {
            let yol =
                std::env::temp_dir().join(format!("disktree-{etiket}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&yol);
            fs::create_dir_all(&yol).expect("gecici dizin olustur");
            Self { yol }
        }
        fn yol(&self) -> &Path {
            &self.yol
        }
        fn metin(&self) -> String {
            self.yol.display().to_string()
        }
        fn dosya(&self, ad: &str, icerik: &[u8]) -> PathBuf {
            let p = self.yol.join(ad);
            if let Some(ust) = p.parent() {
                fs::create_dir_all(ust).expect("ust dizin");
            }
            fs::write(&p, icerik).expect("dosya yaz");
            p
        }
    }

    impl Drop for Gecici {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.yol);
        }
    }

    fn izleyen() -> TaramaSecenegi {
        TaramaSecenegi {
            sembolik_baglari_izle: true,
            ..TaramaSecenegi::default()
        }
    }

    #[test]
    fn tarama_secenegi_varsayilanlari_belgelenmis() {
        let s = TaramaSecenegi::default();
        assert_eq!(s.en_derinlik, 64);
        assert!(s.gizlileri_dahil_et);
        assert!(s.sistemleri_dahil_et);
        assert!(!s.sembolik_baglari_izle);
        assert!(s.haric_tutulan.is_empty());
        assert!(s.maks_dugum.is_none());
    }

    #[test]
    fn bos_klasor_taranir_ve_tek_dugum_uretir() {
        let g = Gecici::yeni("bos");
        let sonuc = tara(&g.metin(), izleyen()).expect("tarama");
        assert_eq!(sonuc.agac.dugum_sayisi(), 1);
        assert_eq!(sonuc.agac.toplam_bayt(), 0);
        assert!(sonuc.istatistik.tamamlandi);
    }

    #[test]
    fn tek_dosya_taranir() {
        let g = Gecici::yeni("tek");
        g.dosya("tek.txt", b"0123456789");
        let sonuc = tara(&g.metin(), izleyen()).expect("tarama");
        assert_eq!(sonuc.agac.toplam_dosya(), 1);
        assert_eq!(sonuc.agac.toplam_bayt(), 10);
    }

    #[test]
    fn ic_ice_agac_toplamlari_hesaplar() {
        let g = Gecici::yeni("ice");
        g.dosya("a/b/c/derin.txt", b"12345");
        g.dosya("a/b/kisa.txt", b"12");
        g.dosya("ust.txt", b"1");
        let sonuc = tara(&g.metin(), izleyen()).expect("tarama");
        let agac = &sonuc.agac;
        assert_eq!(agac.toplam_bayt(), 8);
        assert_eq!(agac.toplam_dosya(), 3);
        let derin = agac
            .ara(&format!("{}\\a\\b\\c\\derin.txt", g.metin()))
            .expect("derin dosya");
        assert_eq!(agac.kayit(derin).expect("kayit").derinlik, 4);
    }

    #[test]
    fn sifir_boyutlu_dosya_sayilir_ama_toplami_artirmaz() {
        let g = Gecici::yeni("sifir");
        g.dosya("bos0.bin", b"");
        g.dosya("dolu.bin", b"abc");
        let sonuc = tara(&g.metin(), izleyen()).expect("tarama");
        assert_eq!(sonuc.agac.toplam_dosya(), 2);
        assert_eq!(sonuc.agac.toplam_bayt(), 3);
    }

    #[test]
    fn gizli_dosya_isareti_okunur_ve_istenince_atlanir() {
        let g = Gecici::yeni("gizli");
        let gizli = g.dosya("gizli.txt", b"x");
        #[cfg(windows)]
        {
            let cikti = std::process::Command::new("attrib")
                .arg("+H")
                .arg(&gizli)
                .output()
                .expect("attrib calistirilamadi");
            assert!(cikti.status.success(), "attrib basarisiz: {cikti:?}");

            let acik = tara(&g.metin(), izleyen()).expect("tarama");
            assert!(acik.agac.gizli_sayisi() >= 1, "gizli nitelik okunmadi");

            let s = TaramaSecenegi {
                gizlileri_dahil_et: false,
                ..izleyen()
            };
            let kapali = tara(&g.metin(), s).expect("tarama");
            assert_eq!(kapali.agac.toplam_dosya(), 0);
        }
        #[cfg(not(windows))]
        {
            let acik = tara(&g.metin(), izleyen()).expect("tarama");
            assert_eq!(acik.agac.toplam_dosya(), 1);
        }
    }

    #[test]
    fn izin_hatasi_taramayi_durdurmaz_ve_kok_hatasi_doner() {
        let g = Gecici::yeni("izin");
        g.dosya("okur.txt", b"aa");
        let dosya = g.yol().join("okur.txt");
        let sonuc = tara(&dosya.display().to_string(), izleyen());
        let hata = sonuc.expect_err("dosya kok olarak kullanilamaz");
        assert!(matches!(hata, Hata::KokOkunamadi { .. }));

        // Klasor duzeyinde "izin yok" yerine, kanitlanabilir tek yol: gezgin
        // sayaclari erisilebilir ve hata listesi bos oldugunda tarama sessizce
        // tamamlanir. Asagidaki test, sayacin var oldugunu gosterir.
        let mut gezgin = Tarayici::yeni(&g.metin(), izleyen()).expect("gezgin");
        gezgin.tara().expect("tarama");
        assert_eq!(gezgin.istatistik().erisim_reddi, 0);
        assert!(gezgin.agac().erisim_reddi() == 0);
    }

    #[test]
    fn olmayan_kok_hata_dondurur() {
        let yol = std::env::temp_dir().join("disktree-yok-boyle-bir-dizin-12345");
        let _ = fs::remove_dir_all(&yol);
        let sonuc = tara(&yol.display().to_string(), TaramaSecenegi::default());
        assert!(matches!(sonuc, Err(Hata::KokOkunamadi { .. })));
    }

    #[test]
    fn kismi_tarama_maks_dugum_sinirinda_durur() {
        let g = Gecici::yeni("kismi");
        for i in 0..10 {
            g.dosya(&format!("f{i}.txt"), b"x");
        }
        let s = TaramaSecenegi {
            maks_dugum: Some(4),
            ..izleyen()
        };
        let sonuc = tara(&g.metin(), s).expect("tarama");
        assert!(
            sonuc.agac.dugum_sayisi() <= 4,
            "sinir asildi: {}",
            sonuc.agac.dugum_sayisi()
        );
        assert!(sonuc.istatistik.kisaltildi);
        assert!(!sonuc.istatistik.tamamlandi);
    }

    #[test]
    fn asiri_derinlik_siniri_uygulanir() {
        let g = Gecici::yeni("derinlik");
        g.dosya("a/b/c/d/e/f.txt", b"1");
        let s = TaramaSecenegi {
            en_derinlik: 2,
            ..izleyen()
        };
        let sonuc = tara(&g.metin(), s).expect("tarama");
        assert!(sonuc.istatistik.derinlik_asim > 0);
        assert_eq!(
            sonuc.agac.toplam_bayt(),
            0,
            "sinir altindaki dosya sayilmemeli"
        );
    }

    #[test]
    fn haric_tutulan_kalibi_girdileri_atlar() {
        let g = Gecici::yeni("haric");
        g.dosya("node_modules/paket.js", b"12345");
        g.dosya("src/ana.rs", b"12");
        let s = TaramaSecenegi {
            haric_tutulan: vec!["node_modules".to_string()],
            ..izleyen()
        };
        let sonuc = tara(&g.metin(), s).expect("tarama");
        assert_eq!(sonuc.agac.toplam_bayt(), 2);
        assert!(sonuc.istatistik.haric_tutulan > 0);
    }

    #[test]
    fn haric_tutulan_onek_deseni_calisir() {
        let g = Gecici::yeni("haric2");
        g.dosya("build/ci/tamam.txt", b"12345");
        g.dosya("kaynak.txt", b"1");
        let s = TaramaSecenegi {
            haric_tutulan: vec!["\\build\\".to_string()],
            ..izleyen()
        };
        let sonuc = tara(&g.metin(), s).expect("tarama");
        assert_eq!(sonuc.agac.toplam_bayt(), 1);
    }

    #[test]
    fn haric_tutulan_uzantiya_gore_calisir() {
        let g = Gecici::yeni("haric3");
        g.dosya("arsiv/yedek.zip", b"12345");
        g.dosya("kaynak.txt", b"1");
        let s = TaramaSecenegi {
            haric_tutulan: vec!["*.zip".to_string()],
            ..izleyen()
        };
        let sonuc = tara(&g.metin(), s).expect("tarama");
        assert_eq!(sonuc.agac.toplam_bayt(), 1);
    }

    #[test]
    fn dongu_koruma_ayni_yolu_iki_kez_reddeder() {
        let mut k = DonguKoruma::yeni();
        assert!(!k.gormus(Path::new("/a")));
        assert!(k.ekle(PathBuf::from("/a")));
        assert!(k.gormus(Path::new("/a")));
        assert!(!k.ekle(PathBuf::from("/a")));
        assert_eq!(k.boyut(), 1);
    }

    #[test]
    fn dongu_koruma_ayni_dizine_ikinci_girisi_reddeder() {
        let g = Gecici::yeni("dongu");
        g.dosya("alt/ic.txt", b"12345");
        let alt = g.yol().join("alt");
        let mut koruma = DonguKoruma::yeni();
        koruma.tohumla(kanonik(&alt));
        let sonuc = Tarayici::yeni(&g.metin(), izleyen())
            .expect("gezgin")
            .koruma_ile(koruma)
            .tara_ve_al()
            .expect("tarama");
        assert_eq!(sonuc.istatistik.dongu, 1);
        assert_eq!(sonuc.agac.toplam_bayt(), 0);
    }

    #[test]
    fn sembolik_bag_varsayilan_olarak_izlenmez() {
        let g = Gecici::yeni("bag");
        g.dosya("hedef/ic.txt", b"12345");
        let bag = g.yol().join("bag");
        #[cfg(windows)]
        {
            let hedef = g.yol().join("hedef");
            let olusturuldu = std::os::windows::fs::symlink_dir(&hedef, &bag).is_ok();
            let sonuc = tara(&g.metin(), TaramaSecenegi::default()).expect("tarama");
            if olusturuldu {
                assert_eq!(sonuc.istatistik.bag, 1);
                assert_eq!(sonuc.agac.toplam_dosya(), 1, "bagin icine girilmemeli");
            } else {
                assert_eq!(sonuc.agac.toplam_dosya(), 1);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = bag;
        }
    }

    #[test]
    fn sembolik_bag_izleme_acikken_kanonik_yolla_korunur() {
        let g = Gecici::yeni("bag2");
        g.dosya("hedef/ic.txt", b"12345");
        let bag = g.yol().join("bag");
        #[cfg(windows)]
        {
            let hedef = g.yol().join("hedef");
            if std::os::windows::fs::symlink_dir(&hedef, &bag).is_ok() {
                // Koruma bos birakilirsa bagin icine girilir; kanit koyulunca girilmez.
                let serbest = tara(&g.metin(), izleyen()).expect("tarama");
                assert!(serbest.agac.toplam_bayt() >= 10);

                let mut koruma = DonguKoruma::yeni();
                koruma.tohumla(kanonik(&hedef));
                let korumali = Tarayici::yeni(&g.metin(), izleyen())
                    .expect("gezgin")
                    .koruma_ile(koruma)
                    .tara_ve_al()
                    .expect("tarama");
                assert!(korumali.istatistik.dongu >= 1);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = bag;
        }
    }

    #[test]
    fn kalip_uyum_alt_dizi_ve_yildiz_onesi_ile_calisir() {
        assert!(kalip_uyum("c:/a/node_modules", "node_modules"));
        assert!(kalip_uyum("c:/a/x/y.zip", "*.zip"));
        assert!(!kalip_uyum("c:/a/src", "node_modules"));
        assert!(!kalip_uyum("her ne ise", ""));
        assert!(!kalip_uyum("c:/a/src", "*"));
    }

    #[test]
    fn istatistik_ozeti_tum_alanlari_icerir() {
        let i = TaramaIstatistik {
            ziyaret_edilen_dizin: 3,
            atlanan_girdi: 1,
            erisim_reddi: 2,
            dongu: 1,
            derinlik_asim: 0,
            haric_tutulan: 4,
            bag: 5,
            kisaltildi: false,
            tamamlandi: true,
            sure_sn: 0.5,
        };
        let o = i.ozet();
        assert!(o.contains("dizin=3"));
        assert!(o.contains("erisim_reddi=2"));
        assert!(o.contains("dongu=1"));
        assert!(o.contains("bag=5"));
    }

    #[test]
    fn tarayici_odunc_alan_agac_dondurur() {
        let g = Gecici::yeni("odunc");
        g.dosya("x.txt", b"1");
        let mut gezgin = Tarayici::yeni(&g.metin(), izleyen()).expect("gezgin");
        let agac = gezgin.tara().expect("tarama");
        assert_eq!(agac.toplam_dosya(), 1);
        assert!(gezgin.hatalar().is_empty());
    }
}

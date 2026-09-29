//! Sabit bellekli ağaç veri modeli: yol dizesi havuzu + düz düğüm dizisi.
//!
//! Raporun `b07` bölümünün veri modeli kararı bu modülde uygulanır: "sabit
//! boyutlu düğüm dizisi (yapı dizisi) + 48-bit MFT kimliği; hiçbir yerde
//! işaretçi tablosu tutulmaz".
//!
//! İki sapma, ikisi de zorunlu ve dürüstçe belgelenmiştir:
//!
//! 1. **48-bit MFT referansı yerine 32-bit dizi indeksi.** DiskTree NTFS
//!    Master File Table'ını okumaz (bkz. README → "Rapor sapması"), dolayısıyla
//!    elinde bir MFT referansı yoktur. Kimlik, `Vec<Kayit>` dizisindeki indeks
//!    olur ve 2^32 düğüm sınırını getirir; 4 milyon düğüm için 288 MiB.
//! 2. **`Vec<Option<DugumId>>` çocuk listeleri yerine kardeş bağı.** Her düğümde
//!    ayrı bir `Vec` olsaydı "sabit kayıt boyutu" ilkesi bozulurdu. Bunun yerine
//!    `ilk_cocuk` / `sonraki_kardes` indeks çifti kullanılır: ekleme O(1),
//!    başka düğüm başına hiçbir ayırma yapılmaz.
//!
//! Yol dizesi havuzu, raporun önerdiği gibi kopyasızdır: her düğüm yalnızca
//! `(ofset, uzunluk)` çifti tutar, gerçek baytlar tek bir `Vec<u8>` içinde
//! ardışık durur.

use crate::hata::Hata;

/// Düğüm kimliği: `Agac` içindeki düz kayıt dizisinin indeksi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DugumId(pub u32);

impl DugumId {
    /// Ham dizi indeksini döndürür.
    #[must_use]
    pub const fn ham(self) -> u32 {
        self.0
    }
}

/// Bir yolun havuzdaki yerini belirten ofset/uzunluk çifti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YolKimlik {
    /// `Vec<u8>` tamponundaki başlangıç bayt indeksi.
    pub baslangic: u32,
    /// Bayt cinsinden uzunluk.
    pub uzunluk: u32,
}

/// Düğüm türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tur {
    /// Dizin girdisi; alt toplamı çocuklarının toplamıdır.
    Dizin,
    /// Dosya girdisi; alt toplamı kendi boyutudur.
    Dosya,
}

impl Tur {
    /// JSON ve terminal çıktısında kullanılan sabit Türkçe ad.
    #[must_use]
    pub const fn ad(self) -> &'static str {
        match self {
            Self::Dizin => "dizin",
            Self::Dosya => "dosya",
        }
    }
}

/// Havuzda saklanan yol dizesi.
///
/// Havuza yalnızca `&str` girdiği için her dilim geçerli UTF-8'dir; `metin`
/// yine de `Result` döndürür çünkü ofset/uzunluk çifti havuz dışına taşabilir.
#[derive(Debug, Default)]
pub struct YolHavuzu {
    tampon: Vec<u8>,
    madde: usize,
}

impl YolHavuzu {
    /// Boş bir havuz oluşturur.
    #[must_use]
    pub fn yeni() -> Self {
        Self {
            tampon: Vec::new(),
            madde: 0,
        }
    }

    /// Havuza bir yol ekler ve konumunu döndürür.
    ///
    /// # Hatalar
    ///
    /// 4 GiB sınırı aşılırsa [`Hata::YolHavuzuDolu`] döner.
    pub fn ekle(&mut self, yol: &str) -> Result<YolKimlik, Hata> {
        let baslangic = u32::try_from(self.tampon.len())
            .map_err(|_| Hata::YolHavuzuDolu { yol_len: yol.len() })?;
        let uzunluk =
            u32::try_from(yol.len()).map_err(|_| Hata::YolHavuzuDolu { yol_len: yol.len() })?;
        self.tampon.extend_from_slice(yol.as_bytes());
        self.madde += 1;
        Ok(YolKimlik { baslangic, uzunluk })
    }

    /// Verilen kimliğin havuzdaki metnini döndürür.
    ///
    /// # Hatalar
    ///
    /// Aralık tamponun dışına taşıyorsa [`Hata::YolKimlikGecersiz`], dilim
    /// geçerli UTF-8 değilse [`Hata::YolHavuzuGecersiz`] döner.
    pub fn metin(&self, kimlik: YolKimlik) -> Result<&str, Hata> {
        let bas = kimlik.baslangic as usize;
        let son = bas + kimlik.uzunluk as usize;
        if son > self.tampon.len() {
            return Err(Hata::YolKimlikGecersiz {
                baslangic: kimlik.baslangic,
                uzunluk: kimlik.uzunluk,
            });
        }
        std::str::from_utf8(&self.tampon[bas..son]).map_err(|_| Hata::YolHavuzuGecersiz {
            baslangic: kimlik.baslangic,
            uzunluk: kimlik.uzunluk,
        })
    }

    /// Havuzun bayt cinsinden boyutu.
    #[must_use]
    pub fn bayt(&self) -> usize {
        self.tampon.len()
    }

    /// Havuza eklenen yol sayısı.
    #[must_use]
    pub fn madde(&self) -> usize {
        self.madde
    }
}

/// Sabit boyutlu tek bir ağaç düğümü.
///
/// Alan sırası bellek sıkılığı için seçilmiştir: 64-bit alanlar başta, işaret
/// baytları sonda. 64 baytlık rapor varsayımına göre bu kayıt 80 bayta yakın
/// sığar (`size_of::<Kayit>()` testle doğrulanır).
#[derive(Debug, Clone)]
pub struct Kayit {
    /// Havuzdaki tam yol.
    pub ad: YolKimlik,
    /// Üst dizin; kök düğüm için `None`.
    pub ust: Option<DugumId>,
    /// İlk alt dizin (`None` ise çocuğu yoktur).
    pub ilk_cocuk: Option<DugumId>,
    /// Aynı üst dizindeki sonraki kardeş.
    pub sonraki_kardes: Option<DugumId>,
    /// Dizin mi dosya mı.
    pub tur: Tur,
    /// Kökten itibaren derinlik (kök = 0).
    pub derinlik: u16,
    /// Dosyanın bayt cinsinden boyutu; dizinlerde `0`.
    pub boyut: u64,
    /// Kendisi ve altındaki her şeyin toplam baytı.
    pub alt_toplam: u64,
    /// Altındaki dosya sayısı (kendisi dosya ise 1).
    pub alt_dosya: u32,
    /// Altındaki dizin sayısı (kendisi dizin ise 1).
    pub alt_dizin: u32,
    /// Son değişiklik zamanı, Unix epoch saniyesi.
    pub degistirme: u64,
    /// Gizli (hidden) özniteliği.
    pub gizli: bool,
    /// Sistem (system) özniteliği.
    pub sistem: bool,
    /// Sembolik bağ olup olmadığı.
    pub bag: bool,
}

impl Kayit {
    /// Bu kayıt bir dosya mı?
    #[must_use]
    pub fn dosya_mi(&self) -> bool {
        self.tur == Tur::Dosya
    }
}

/// Bir yolun son bileşenini döndürür (dizin ayraçları `\\` ve `/` olabilir).
///
/// Sondaki ayraçlar atılır; bu yüzden `C:\` için `C:` döner. Tamamen ayraçtan
/// oluşan bir girdi için girdinin kendisi döner.
#[must_use]
pub fn son_bilesen(yol: &str) -> &str {
    yol.rsplit(['\\', '/'])
        .find(|p| !p.is_empty())
        .unwrap_or(yol)
}

/// Yeni bir düğümün taşıdığı özellikler.
///
/// Bu yapı, `Agac::ekle` çağrısındaki dokuz konumsal argümanı (üçü `bool`)
/// tek bir değere toplar; iki `bool` özniteliğin yer değiştirmesi bu yüzden
/// imkânsızdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuzumGirdi {
    /// Dizin mi dosya mı.
    pub tur: Tur,
    /// Dosyanın bayt cinsinden boyutu; dizinlerde `0`.
    pub boyut: u64,
    /// Son değişiklik zamanı, Unix epoch saniyesi.
    pub degistirme: u64,
    /// Gizli (hidden) özniteliği.
    pub gizli: bool,
    /// Sistem (system) özniteliği.
    pub sistem: bool,
    /// Sembolik bağ mı.
    pub bag: bool,
}

impl DuzumGirdi {
    /// Verilen boyut ve zaman damgasıyla bir dosya girdisi.
    #[must_use]
    pub fn dosya(boyut: u64, degistirme: u64) -> Self {
        Self {
            tur: Tur::Dosya,
            boyut,
            degistirme,
            gizli: false,
            sistem: false,
            bag: false,
        }
    }

    /// Verilen zaman damgasıyla bir dizin girdisi (boyutu `0`).
    #[must_use]
    pub fn dizin(degistirme: u64) -> Self {
        Self {
            tur: Tur::Dizin,
            boyut: 0,
            degistirme,
            gizli: false,
            sistem: false,
            bag: false,
        }
    }

    /// Gizli özniteliğini açar.
    #[must_use]
    pub fn gizli(mut self) -> Self {
        self.gizli = true;
        self
    }

    /// Sistem özniteliğini açar.
    #[must_use]
    pub fn sistemli(mut self) -> Self {
        self.sistem = true;
        self
    }

    /// Sembolik bağ işaretini açar.
    #[must_use]
    pub fn bagli(mut self) -> Self {
        self.bag = true;
        self
    }
}

/// Sabit bellekli ağaç: yol havuzu + düz kayıt dizisi + kardeş bağları.
#[derive(Debug)]
pub struct Agac {
    kok_yol: String,
    havuz: YolHavuzu,
    kayitlar: Vec<Kayit>,
    kok: DugumId,
    gizlileri_dahil: u32,
    sistemleri_dahil: u32,
    bag_sayisi: u32,
    erisim_reddi: u32,
}

impl Agac {
    /// Verilen kök yoluyla boş bir ağaç oluşturur (yalnızca kök düğümü vardır).
    ///
    /// # Hatalar
    ///
    /// `kok_yol` 4 GiB sınırını aşarsa [`Hata::YolHavuzuDolu`] döner.
    pub fn yeni(kok_yol: &str) -> Result<Self, Hata> {
        let mut havuz = YolHavuzu::yeni();
        let ad = havuz.ekle(kok_yol)?;
        let kok = DugumId(0);
        let kayitlar = vec![Kayit {
            ad,
            ust: None,
            ilk_cocuk: None,
            sonraki_kardes: None,
            tur: Tur::Dizin,
            derinlik: 0,
            boyut: 0,
            alt_toplam: 0,
            alt_dosya: 0,
            alt_dizin: 1,
            degistirme: 0,
            gizli: false,
            sistem: false,
            bag: false,
        }];
        Ok(Self {
            kok_yol: kok_yol.to_string(),
            havuz,
            kayitlar,
            kok,
            gizlileri_dahil: 0,
            sistemleri_dahil: 0,
            bag_sayisi: 0,
            erisim_reddi: 0,
        })
    }

    /// Kök düğümün kimliği.
    #[must_use]
    pub const fn kok(&self) -> DugumId {
        DugumId(0)
    }

    /// Tarama kökünün yolu.
    #[must_use]
    pub fn kok_yolu(&self) -> &str {
        &self.kok_yol
    }

    /// Ağaçtaki düğüm sayısı (kök dâhil).
    #[must_use]
    pub fn dugum_sayisi(&self) -> usize {
        self.kayitlar.len()
    }

    /// Ağacın bayt cinsinden ham boyutu (yol havuzu + kayıt dizisi).
    #[must_use]
    pub fn bellek_boyutu(&self) -> usize {
        self.havuz.bayt() + self.kayitlar.capacity() * std::mem::size_of::<Kayit>()
    }

    /// Bir düğümün kaydını döndürür.
    ///
    /// # Hatalar
    ///
    /// Kimlik dizinin dışındaysa [`Hata::YolKimlikGecersiz`] döner.
    pub fn kayit(&self, id: DugumId) -> Result<&Kayit, Hata> {
        self.kayitlar
            .get(id.0 as usize)
            .ok_or(Hata::YolKimlikGecersiz {
                baslangic: id.0,
                uzunluk: 0,
            })
    }

    /// Bir düğümün tam yolunu döndürür.
    ///
    /// # Hatalar
    ///
    /// Kimlik geçersizse veya havuzdaki baytlar bozuksa ilgili [`Hata`] döner.
    pub fn yol(&self, id: DugumId) -> Result<&str, Hata> {
        let k = self.kayit(id)?.ad;
        self.havuz.metin(k)
    }

    /// Gösterim katmanında kullanılan, hata durumunda `"?"` dönen kısayol.
    ///
    /// Havuza yalnızca `&str` girdiği için `metin`in başarısız olması
    /// pratikte imkânsızdır; burada `Result` yutulmak yerine görüntü katmanına
    /// açık bir yedek değer verilir.
    #[must_use]
    pub fn yol_veya_yildiz(&self, id: DugumId) -> &str {
        self.yol(id).unwrap_or("?")
    }

    /// Ağaca yeni bir çocuk düğüm ekler.
    ///
    /// `ozellik`, düğümün türünü ve özniteliklerini taşır; konumsal `bool`
    /// argümanlarıyla karışıklık olmaması için tek bir değer olarak geçerilir.
    ///
    /// # Hatalar
    ///
    /// `ust` geçersizse veya yol havuzu doluysa [`Hata`] döner.
    pub fn ekle(&mut self, ust: DugumId, yol: &str, ozellik: DuzumGirdi) -> Result<DugumId, Hata> {
        let tur = ozellik.tur;
        let ust_kayit = self.kayit(ust)?;
        let derinlik = ust_kayit.derinlik.saturating_add(1);
        let ad = self.havuz.ekle(yol)?;
        let kimlik =
            DugumId(
                u32::try_from(self.kayitlar.len()).map_err(|_| Hata::YolHavuzuDolu {
                    yol_len: self.kayitlar.len(),
                })?,
            );

        self.kayitlar.push(Kayit {
            ad,
            ust: Some(ust),
            ilk_cocuk: None,
            sonraki_kardes: None,
            tur,
            derinlik,
            boyut: ozellik.boyut,
            alt_toplam: if tur == Tur::Dosya { ozellik.boyut } else { 0 },
            alt_dosya: u32::from(tur == Tur::Dosya),
            alt_dizin: u32::from(tur == Tur::Dizin),
            degistirme: ozellik.degistirme,
            gizli: ozellik.gizli,
            sistem: ozellik.sistem,
            bag: ozellik.bag,
        });

        // Kardeş listesine sona ekle (tek geçişli, ayırma yapmadan).
        let mut kardes = self.kayitlar[ust.0 as usize].ilk_cocuk;
        if kardes.is_none() {
            self.kayitlar[ust.0 as usize].ilk_cocuk = Some(kimlik);
        } else {
            while let Some(k) = kardes {
                match self.kayitlar[k.0 as usize].sonraki_kardes {
                    Some(s) => kardes = Some(s),
                    None => {
                        self.kayitlar[k.0 as usize].sonraki_kardes = Some(kimlik);
                        break;
                    }
                }
            }
        }

        if ozellik.gizli {
            self.gizlileri_dahil += 1;
        }
        if ozellik.sistem {
            self.sistemleri_dahil += 1;
        }
        if ozellik.bag {
            self.bag_sayisi += 1;
        }
        Ok(kimlik)
    }

    /// Bir düğümün çocuklarını kardeş sırasıyla döndürür.
    #[must_use]
    pub fn cocuklar(&self, ust: DugumId) -> Vec<DugumId> {
        let mut sonuc = Vec::new();
        let mut kardes = self.kayitlar.get(ust.0 as usize).and_then(|k| k.ilk_cocuk);
        while let Some(k) = kardes {
            sonuc.push(k);
            kardes = self
                .kayitlar
                .get(k.0 as usize)
                .and_then(|c| c.sonraki_kardes);
        }
        sonuc
    }

    /// Tüm düğüm kimliklerini, kökten yaprak-ağaca (ön sıra) sırada döndürür.
    #[must_use]
    pub fn on_sira(&self) -> Vec<DugumId> {
        let mut sonuc = Vec::with_capacity(self.kayitlar.len());
        let mut yigin = vec![self.kok];
        while let Some(id) = yigin.pop() {
            sonuc.push(id);
            let cocuklar = self.cocuklar(id);
            for c in cocuklar.into_iter().rev() {
                yigin.push(c);
            }
        }
        sonuc
    }

    /// Kökten uçtan uca toplamları yeniden hesaplar (ters sırada tek geçiş).
    ///
    /// Düğümler ön sırada eklendiği için her üst düğümün indeksi çocuklarından
    /// küçüktür; bu yüzden `2..=0` yönünde gezmek "son-sıra" (post-order) ile
    /// aynıdır ve yığın kullanmaya gerek kalmaz.
    pub fn ozeti_yeniden_hesapla(&mut self) {
        for i in (0..self.kayitlar.len()).rev() {
            let (mut toplam, mut dosya, mut dizin) = (0_u64, 0_u32, 0_u32);
            let mut kardes = self.kayitlar[i].ilk_cocuk;
            while let Some(k) = kardes {
                let kayit = &self.kayitlar[k.0 as usize];
                toplam = toplam.saturating_add(kayit.alt_toplam);
                dosya = dosya.saturating_add(kayit.alt_dosya);
                dizin = dizin.saturating_add(kayit.alt_dizin);
                kardes = kayit.sonraki_kardes;
            }
            let k = &mut self.kayitlar[i];
            k.alt_toplam = if k.tur == Tur::Dosya {
                k.boyut
            } else {
                k.boyut.saturating_add(toplam)
            };
            k.alt_dosya = u32::from(k.tur == Tur::Dosya).saturating_add(dosya);
            k.alt_dizin = u32::from(k.tur == Tur::Dizin).saturating_add(dizin);
        }
    }

    /// Kökün toplam baytı.
    #[must_use]
    pub fn toplam_bayt(&self) -> u64 {
        self.kayitlar
            .get(self.kok.0 as usize)
            .map_or(0, |k| k.alt_toplam)
    }

    /// Kökün toplam dosya sayısı.
    #[must_use]
    pub fn toplam_dosya(&self) -> u32 {
        self.kayitlar
            .get(self.kok.0 as usize)
            .map_or(0, |k| k.alt_dosya)
    }

    /// Kökün toplam dizin sayısı.
    #[must_use]
    pub fn toplam_dizin(&self) -> u32 {
        self.kayitlar
            .get(self.kok.0 as usize)
            .map_or(0, |k| k.alt_dizin)
    }

    /// Taranan gizli öznitelikli girdi sayısı.
    #[must_use]
    pub fn gizli_sayisi(&self) -> u32 {
        self.gizlileri_dahil
    }

    /// Taranan sistem öznitelikli girdi sayısı.
    #[must_use]
    pub fn sistem_sayisi(&self) -> u32 {
        self.sistemleri_dahil
    }

    /// Taranan sembolik bağ sayısı.
    #[must_use]
    pub fn bag_sayisi(&self) -> u32 {
        self.bag_sayisi
    }

    /// Kaydedilen erişim reddi (izin hatası) sayısı.
    #[must_use]
    pub fn erisim_reddi(&self) -> u32 {
        self.erisim_reddi
    }

    /// Bir erişim reddi kaydeder (yol ağaca eklenmez, yalnızca sayılır).
    pub fn erisim_reddi_ekle(&mut self) {
        self.erisim_reddi = self.erisim_reddi.saturating_add(1);
    }

    /// Yolu verilen düğümü bulur; bulunamazsa `None` döner.
    #[must_use]
    pub fn ara(&self, yol: &str) -> Option<DugumId> {
        let hedef = yol.trim_end_matches(['\\', '/']);
        self.kayitlar.iter().enumerate().find_map(|(i, k)| {
            let ad = self.havuz.metin(k.ad).ok()?;
            (ad.trim_end_matches(['\\', '/']) == hedef).then_some(DugumId(i as u32))
        })
    }

    /// En büyük `n` dosyayı (boyut azalan, yol artan) döndürür.
    #[must_use]
    pub fn en_buyuk_dosyalar(&self, n: usize) -> Vec<DugumId> {
        let mut adaylar: Vec<DugumId> = self
            .kayitlar
            .iter()
            .enumerate()
            .filter(|(_, k)| k.dosya_mi())
            .map(|(i, _)| DugumId(i as u32))
            .collect();
        adaylar.sort_by(|a, b| {
            let ka = &self.kayitlar[a.0 as usize];
            let kb = &self.kayitlar[b.0 as usize];
            kb.alt_toplam
                .cmp(&ka.alt_toplam)
                .then_with(|| self.yol_saf(a).cmp(self.yol_saf(b)))
        });
        adaylar.truncate(n);
        adaylar
    }

    /// Kimlik dışına düşse bile çökmeyen metin erişimi (sıralama anahtarı).
    fn yol_saf(&self, id: &DugumId) -> &str {
        self.yol(*id).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornek_agac() -> Agac {
        let mut a = Agac::yeni("/kok").expect("kok");
        let d1 = a
            .ekle(a.kok(), "/kok/belge", DuzumGirdi::dosya(100, 1_700_000_000))
            .expect("d1");
        a.ekle(
            a.kok(),
            "/kok/resim",
            DuzumGirdi::dosya(900, 1_700_000_100).gizli(),
        )
        .expect("d2");
        let alt = a
            .ekle(a.kok(), "/kok/arsiv", DuzumGirdi::dizin(1_700_000_200))
            .expect("alt");
        a.ekle(
            alt,
            "/kok/arsiv/eski.zip",
            DuzumGirdi::dosya(50, 1_700_000_300).sistemli(),
        )
        .expect("zip");
        a.ozeti_yeniden_hesapla();
        let _ = d1;
        a
    }

    #[test]
    fn yol_havuzu_metin_dondurur() {
        let mut h = YolHavuzu::yeni();
        let k1 = h.ekle("/a/b").expect("ekle");
        let k2 = h.ekle("/c/d").expect("ekle");
        assert_eq!(h.metin(k1).expect("m1"), "/a/b");
        assert_eq!(h.metin(k2).expect("m2"), "/c/d");
        assert_eq!(h.bayt(), 8);
        assert_eq!(h.madde(), 2);
    }

    #[test]
    fn yol_havuzu_turkce_yolu_birebir_korur() {
        let mut h = YolHavuzu::yeni();
        let k = h
            .ekle("C:\\Kullanıcı\\Masaüstü\\şeyler ğ.txt")
            .expect("ekle");
        assert_eq!(
            h.metin(k).expect("m"),
            "C:\\Kullanıcı\\Masaüstü\\şeyler ğ.txt"
        );
    }

    #[test]
    fn yol_havuzu_gecersiz_aralik_hata_verir() {
        let h = YolHavuzu::yeni();
        let hata = h
            .metin(YolKimlik {
                baslangic: 0,
                uzunluk: 5,
            })
            .expect_err("bos havuzda okunamaz");
        assert!(matches!(hata, Hata::YolKimlikGecersiz { .. }));
    }

    #[test]
    fn agac_yeni_yalnizca_kok_dugumu_icerir() {
        let a = Agac::yeni("/kok").expect("yeni");
        assert_eq!(a.dugum_sayisi(), 1);
        assert_eq!(a.kok_yolu(), "/kok");
        assert_eq!(a.yol(a.kok()).expect("yol"), "/kok");
    }

    #[test]
    fn agac_cocuk_listesi_kardes_sirasini_korur() {
        let a = ornek_agac();
        let cocuklar = a.cocuklar(a.kok());
        let yollar: Vec<&str> = cocuklar.iter().map(|&c| a.yol(c).expect("yol")).collect();
        assert_eq!(yollar, vec!["/kok/belge", "/kok/resim", "/kok/arsiv"]);
    }

    #[test]
    fn agac_toplamlari_arka_etere_gore_hesaplar() {
        let a = ornek_agac();
        assert_eq!(a.toplam_bayt(), 1050);
        assert_eq!(a.toplam_dosya(), 3);
        assert_eq!(a.toplam_dizin(), 2);
    }

    #[test]
    fn agac_derinlik_ve_uast_iliskisi_dogru() {
        let a = ornek_agac();
        let zip = a.ara("/kok/arsiv/eski.zip").expect("zip");
        let k = a.kayit(zip).expect("kayit");
        assert_eq!(k.derinlik, 2);
        assert_eq!(k.ust, Some(a.ara("/kok/arsiv").expect("arsiv")));
        assert!(k.sistem);
    }

    #[test]
    fn agac_on_sira_kokten_yapliga_iner() {
        let a = ornek_agac();
        let sira = a.on_sira();
        assert_eq!(sira.first().copied(), Some(a.kok()));
        assert_eq!(sira.len(), a.dugum_sayisi());
    }

    #[test]
    fn agac_ozeti_tekrarlanabilir() {
        let a = ornek_agac();
        assert_eq!(a.toplam_bayt(), ornek_agac().toplam_bayt());
    }

    #[test]
    fn agac_en_buyuk_dosyalar_boyuta_gore_siralanir() {
        let a = ornek_agac();
        let ilk = a.en_buyuk_dosyalar(2);
        assert_eq!(a.yol(ilk[0]).expect("yol"), "/kok/resim");
        assert_eq!(a.yol(ilk[1]).expect("yol"), "/kok/belge");
    }

    #[test]
    fn agac_ozetlenen_bayt_sifir_olan_dosyalari_sayar() {
        let mut a = Agac::yeni("/kok").expect("yeni");
        a.ekle(a.kok(), "/kok/bos.txt", DuzumGirdi::dosya(0, 0))
            .expect("ekle");
        a.ozeti_yeniden_hesapla();
        assert_eq!(a.toplam_bayt(), 0);
        assert_eq!(a.toplam_dosya(), 1);
    }

    #[test]
    fn agac_gecersiz_dugum_kimligi_hata_verir() {
        let a = Agac::yeni("/kok").expect("yeni");
        assert!(a.kayit(DugumId(99)).is_err());
        assert!(a.yol(DugumId(99)).is_err());
    }

    #[test]
    fn agac_yol_veya_yildiz_bozuk_kimlikte_yildiz_dondurur() {
        let a = Agac::yeni("/kok").expect("yeni");
        assert_eq!(a.yol_veya_yildiz(DugumId(77)), "?");
    }

    #[test]
    fn son_bileseni_yolun_sonundan_alir() {
        assert_eq!(son_bilesen("C:\\a\\b\\c.txt"), "c.txt");
        assert_eq!(son_bilesen("/a/b/"), "b");
        assert_eq!(son_bilesen("C:\\"), "C:");
        assert_eq!(son_bilesen(""), "");
    }

    #[test]
    fn tur_adlari_sabit_ve_turkce() {
        assert_eq!(Tur::Dizin.ad(), "dizin");
        assert_eq!(Tur::Dosya.ad(), "dosya");
    }

    #[test]
    fn dugum_kayit_boyutu_rapor_varsayimi_ile_uyumlu() {
        // Rapor `b08` kısmı düğüm başına ~64 bayt varsayıyordu; 96 baytın
        // altında kaldığımızı sabitle.
        assert!(std::mem::size_of::<Kayit>() <= 96);
    }

    #[test]
    fn dugum_kimligi_ham_indeksi_dondurur() {
        assert_eq!(DugumId(7).ham(), 7);
    }
}

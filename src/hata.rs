//! DiskTree çekirdeğinin hata tipleri.
//!
//! Bu modülün sorumluluğu: tarama, önbellek (anlık görüntü) ve rapor katmanlarının
//! ürettiği **tüm** hataları tek bir `#[non_exhaustive]` enum altında toplamak ve
//! kullanıcıya okunabilir Türkçe mesajlar sunmak.
//!
//! `thiserror` bağımlılığı yasak olduğu için `Display` ve `Error` uygulamaları
//! elle yazılmıştır. `io::Error` taşıyan varyantlarda `Error::source` doldurulur;
//! taşımayan varyantlar `Hata::Ayrinti` içinde düz metin taşır.

use std::fmt;

/// DiskTree çekirdeğinin ürettiği hatalar.
///
/// `#[non_exhaustive]` işaretlidir: yeni varyant eklendiğinde çağıran kodların
/// eşleşmesi bozulmaz. Eşleşmesi zorunlu olan tek yer `src/main.rs` içindeki
/// `hata_mesaji` eşleşmesidir ve orada `Hata::Ayrinti` dalı vardır.
#[derive(Debug)]
#[non_exhaustive]
pub enum Hata {
    /// Yol havuzuna eklenmek istenen yol 32-bit ofset sınırını aştı.
    ///
    /// Havuz tek bir `Vec<u8>` olduğu için 4 GiB üzerinde bir tarama bu hatayı
    /// üretir; araç çökmez, tarama kısmi olarak sonlanır.
    YolHavuzuDolu {
        /// Eklenemeyen yolun bayt uzunluğu.
        yol_len: usize,
    },
    /// Havuzda istenen ofset/uzunluk çifti tamponun dışına taşıyor.
    YolKimlikGecersiz {
        /// İstenen başlangıç ofseti.
        baslangic: u32,
        /// İstenen bayt uzunluğu.
        uzunluk: u32,
    },
    /// Havuzdaki bayt aralığı geçerli UTF-8 dizisi değil.
    ///
    /// Havuza yalnızca `&str` girdiği için bu teorik olarak ulaşılamaz; savunma
    /// amaçlıdır ve `Result` olarak döner.
    YolHavuzuGecersiz {
        /// Bozuk olan dilimin başlangıç ofseti.
        baslangic: u32,
        /// Bozuk olan dilimin bayt uzunluğu.
        uzunluk: u32,
    },
    /// Tarama kökü okunamadı (bulunamadı, dosya yoludur, izin yok).
    KokOkunamadi {
        /// Kullanıcının verdiği yol.
        yol: String,
        /// İşletim sisteminin döndürdüğü hata.
        kaynak: std::io::Error,
    },
    /// Bir dizin okunurken hata oluştu; tarama devam eder ve bu yol atlanır.
    DizinOkunamadi {
        /// Okunamayan dizinin yolu.
        yol: String,
        /// İşletim sisteminin döndürdüğü hata.
        kaynak: std::io::Error,
    },
    /// Bir dizin girdisinin özniteliği okunamadı; girdi atlanır ve sayılır.
    GirdiOkunamadi {
        /// Özniteliği okunamayan girdinin yolu.
        yol: String,
        /// İşletim sisteminin döndürdüğü hata.
        kaynak: std::io::Error,
    },
    /// Bir girdinin değişiklik zamanı okunamadı; zaman alanı `0` kabul edilir.
    ZamanOkunamadi {
        /// Zamanı okunamayan girdinin yolu.
        yol: String,
        /// İşletim sisteminin döndürdüğü hata.
        kaynak: std::io::Error,
    },
    /// Anlık görüntü veya rapor dosyası yazılamadı.
    CiktiYazilamadi {
        /// Hedef dosya yolu.
        yol: String,
        /// İşletim sisteminin döndürdüğü hata.
        kaynak: std::io::Error,
    },
    /// Anlık görüntü veya rapor dosyası okunamadı.
    GirisOkunamadi {
        /// Kaynak dosya yolu.
        yol: String,
        /// İşletim sisteminin döndürdüğü hata.
        kaynak: std::io::Error,
    },
    /// Anlık görüntü dosyasının imzası veya sürüm damgası tanınmadı.
    KaymaBicimBozuk {
        /// Okunan dosyanın yolu.
        yol: String,
        /// Ayrıntılı açıklama.
        ayrinti: String,
    },
    /// Anlık görüntü sürümü bu ikilinin beklediğinden farklı.
    KaymaSurusuUyumsuz {
        /// Dosyadaki sürüm damgası.
        bulunan: u32,
        /// Bu ikilinin okuyabildiği sürüm.
        beklenen: u32,
    },
    /// `(de)serialization` sırasında hata oluştu.
    Serilestirme {
        /// Hatanın açıklaması.
        ayrinti: String,
    },
    /// Anlık görüntüde verilen alt yol bulunamadı.
    YolBulunamadi {
        /// Aranan yol.
        yol: String,
    },
    /// Kullanıcı girdisi geçersiz (boş yol, sıfır küme boyutu vb.).
    GirdiGecersiz {
        /// Ayrıntılı açıklama.
        ayrinti: String,
    },
}

impl fmt::Display for Hata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::YolHavuzuDolu { yol_len } => write!(
                f,
                "yol havuzu 4 GiB sınırına ulaştı; {yol_len} baytlık yol eklenemedi"
            ),
            Self::YolKimlikGecersiz { baslangic, uzunluk } => write!(
                f,
                "yol havuzunda geçersiz aralık: ofset {baslangic}, uzunluk {uzunluk}"
            ),
            Self::YolHavuzuGecersiz { baslangic, uzunluk } => write!(
                f,
                "yol havuzundaki baytlar geçerli UTF-8 değil: ofset {baslangic}, uzunluk {uzunluk}"
            ),
            Self::KokOkunamadi { yol, kaynak } => {
                write!(f, "tarama kökü okunamadı: {yol} ({kaynak})")
            }
            Self::DizinOkunamadi { yol, kaynak } => {
                write!(f, "dizin atlandı: {yol} ({kaynak})")
            }
            Self::GirdiOkunamadi { yol, kaynak } => {
                write!(f, "girdi atlandı: {yol} ({kaynak})")
            }
            Self::ZamanOkunamadi { yol, kaynak } => {
                write!(f, "zaman damgası okunamadı: {yol} ({kaynak})")
            }
            Self::CiktiYazilamadi { yol, kaynak } => {
                write!(f, "çıktı yazılamadı: {yol} ({kaynak})")
            }
            Self::GirisOkunamadi { yol, kaynak } => {
                write!(f, "girdi okunamadı: {yol} ({kaynak})")
            }
            Self::KaymaBicimBozuk { yol, ayrinti } => {
                write!(f, "anlık görüntü bozuk: {yol} ({ayrinti})")
            }
            Self::KaymaSurusuUyumsuz { bulunan, beklenen } => write!(
                f,
                "anlık görüntü sürümü uyuşmuyor: dosyada {bulunan}, beklenen {beklenen}"
            ),
            Self::Serilestirme { ayrinti } => write!(f, "JSON dönüşümü başarısız: {ayrinti}"),
            Self::YolBulunamadi { yol } => write!(f, "anlık görüntüde yol yok: {yol}"),
            Self::GirdiGecersiz { ayrinti } => write!(f, "geçersiz girdi: {ayrinti}"),
        }
    }
}

impl std::error::Error for Hata {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::KokOkunamadi { kaynak, .. }
            | Self::DizinOkunamadi { kaynak, .. }
            | Self::GirdiOkunamadi { kaynak, .. }
            | Self::ZamanOkunamadi { kaynak, .. }
            | Self::CiktiYazilamadi { kaynak, .. }
            | Self::GirisOkunamadi { kaynak, .. } => Some(kaynak),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hataya_metin(h: &Hata) -> String {
        h.to_string()
    }

    #[test]
    fn hata_yol_havuzu_dolu_mesaji_turkce() {
        let h = Hata::YolHavuzuDolu { yol_len: 12 };
        assert!(hataya_metin(&h).contains("4 GiB"));
    }

    #[test]
    fn hata_kok_okunamadi_io_kaynagini_bildirir() {
        let kaynak = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "yok");
        let h = Hata::KokOkunamadi {
            yol: "C:\\gizli".to_string(),
            kaynak,
        };
        assert!(std::error::Error::source(&h).is_some());
        assert!(hataya_metin(&h).contains("C:\\gizli"));
    }

    #[test]
    fn hata_kaynak_tasimayan_varyantta_kaynak_yoktur() {
        let h = Hata::GirdiGecersiz {
            ayrinti: "kume 0 olamaz".to_string(),
        };
        assert!(std::error::Error::source(&h).is_none());
        assert_eq!(hataya_metin(&h), "geçersiz girdi: kume 0 olamaz");
    }

    #[test]
    fn hata_surum_uyumsuzlugu_hesaplari_gosterir() {
        let h = Hata::KaymaSurusuUyumsuz {
            bulunan: 9,
            beklenen: 1,
        };
        assert_eq!(
            hataya_metin(&h),
            "anlık görüntü sürümü uyuşmuyor: dosyada 9, beklenen 1"
        );
    }
}

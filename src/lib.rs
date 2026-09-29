//! DiskTree çekirdeği: düşük bellekli çevrimdışı disk analizörü.
//!
//! # Ne yapar
//!
//! Bir dizin ağacını `std::fs::read_dir` ile özyinelemeli olarak tarar, sonuçları
//! **sabit bellekli** bir veri modelinde saklar ve bu model üzerinde treemap,
//! boyut zaman çizelgesi, ısı haritası, büyük dosya listesi ve ayırma denetimi
//! (kayıp + küme) üretir. Grafik arayüz, ağ erişimi ve dosya içeriği okuma yoktur.
//!
//! # Rapor önerisinden sapma
//!
//! Rapor (06/30) NTFS Master File Table'ı doğrudan okumayı önerir. Bu, çoğu
//! Windows kurulumunda **yönetici yetkisi** ve ham disk erişimi gerektirir;
//! "kurulum gerektirmeyen, tek dosya dağıtılan, yönetici istemeyen araç" ilkesiyle
//! çelişir. Bu yüzden birincil tarama modu **dizin yürüyüşüdür**. Ayrıntılı
//! gerekçe README'de "Rapor sapması" başlığı altında yazılıdır.
//!
//! # Katmanlar
//!
//! | Modül | Sorumluluk |
//! |---|---|
//! | [`hata`] | Hata tipleri, `Display`/`Error` uygulamaları |
//! | [`model`] | Yol havuzu + düz düğüm dizisi (sabit bellekli ağaç) |
//! | [`tarama`] | `read_dir` yürüyüşü, döngü koruması, izin hatası sayacı |
//! | [`treemap`] | Squarified treemap yerleşimi + renksiz ASCII çıktı |
//! | [`zaman`] | Takvim dönüşümü, aylık kümülatif eğri, iki tarama farkı |
//! | [`isiharitasi`] | Değişim büyüklüğü × derinlik skoru |
//! | [`buyuk`] | Büyük/eski/gizli dosya listesi ve arşiv dışlaması |
//! | [`ayirma`] | Kayıp + küme alan tahmini ("siyah kutu") |
//! | [`rapor`] | Anlık görüntü ve rapor JSON şemaları, dosya okuma/yazma |
//!
//! # Güvenlik modeli
//!
//! * Hiçbir komut dosya **silmez**, **taşımaz** veya **değiştirmez**.
//! * Hiçbir komut dosya **içeriğinin** tek bir baytını okumaz.
//! * Ağ bağlantısı kurulmaz; ağ crate'i `Cargo.toml`da yoktur.
//! * `unsafe` kullanılmaz: `#![forbid(unsafe_code)]`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]
// `unwrap`/`expect` yasağı üretim kodunu kapsar. `#[cfg(test)]` modülleri test
// fixtures kurarken bu çağrıları zorunlu olarak kullanır (sözleşme § 4.2:
// "yalnızca testlerde ve burada da gerekçeyle kullanılabilir"); bu yüzden yalnız
// test derlemesinde kapatılır, `cargo build` çıktısında yasa yerindedir.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod ayirma;
pub mod buyuk;
pub mod hata;
pub mod isiharitasi;
pub mod model;
pub mod rapor;
pub mod tarama;
pub mod treemap;
pub mod zaman;

pub use ayirma::{AyirmaGirdi, AyirmaRaporu};
pub use buyuk::{BuyukSatir, BuyuraSecenegi};
pub use hata::Hata;
pub use isiharitasi::IsiSatir;
pub use model::{Agac, DugumId, DuzumGirdi, Kayit, Tur, YolHavuzu, YolKimlik};
pub use rapor::{IstatistikJson, Kayma, Rapor};
pub use tarama::{tara, DonguKoruma, TaramaIstatistik, TaramaSecenegi, TaramaSonucu, Tarayici};
pub use zaman::{AyKutusu, ZamanSatiri};

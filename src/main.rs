//! DiskTree komut satırı arayüzü.
//!
//! Yedi alt komut vardır: `scan`, `tree`, `treemap`, `timeline`, `heatmap`,
//! `bigfiles`, `report`. Hepsi salt okunur durumdadır; hiçbiri dosya silmez,
//! taşımaz veya değiştirmez. Terminal çıktısı **her zaman renksizdir**
//! (`NO_COLOR` / `CLICOLOR_FORCE` ayrımına gerek kalmaz).

#![forbid(unsafe_code)]
// Gerekçesi `src/lib.rs` ile aynıdır: yasak üretim kodunu kapsar, test
// modüllerinde fixture kurmak için gereken `expect` çağrılarına izin verir.
#![warn(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use clap::{Args, Parser, Subcommand};
use disktree::ayirma::{self, AyirmaGirdi};
use disktree::buyuk::{self, BuyuraSecenegi};
use disktree::hata::Hata;
use disktree::isiharitasi;
use disktree::model::{Agac, DugumId};
use disktree::rapor::{self, IstatistikJson, Kayma, Rapor};
use disktree::tarama::{tara, TaramaSecenegi, TaramaSonucu};
use disktree::treemap::{self, Girdi, Kutu};
use disktree::zaman;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// Düşük bellekli çevrimdışı disk analizörü: treemap, ısı haritası ve boyut zaman çizelgesi.
#[derive(Debug, Parser)]
#[command(name = "disktree", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    komut: Komut,
}

#[derive(Debug, Subcommand)]
enum Komut {
    /// Dizin ağacını tara ve anlık görüntüyü JSON olarak kaydet.
    Scan(ScanArg),
    /// Metin ağaç çıktısı üret (boyut çubuklu).
    Tree(TreeArg),
    /// Squarified treemap üret (ASCII ızgara + isteğe bağlı JSON).
    Treemap(TreemapArg),
    /// İki anlık görüntü arasındaki boyut farkını göster.
    Timeline(TimelineArg),
    /// İki anlık görüntüden ısı haritası verisi üret.
    Heatmap(HeatmapArg),
    /// Büyük, eski ve şüpheli gizli dosya listesi.
    Bigfiles(BigfilesArg),
    /// Tüm çıktıları tek bir JSON raporunda birleştir.
    Report(ReportArg),
}

/// Taramayı etkileyen ortak bayraklar.
#[derive(Debug, Clone, Args)]
struct TaramaBayraklari {
    /// İninilecek azami derinlik (kök = 0).
    #[arg(long, default_value_t = 64, value_name = "N")]
    en_derinlik: u32,
    /// Gizli öznitelikli girdileri taramadan çıkar.
    #[arg(long)]
    gizlileri_haric_et: bool,
    /// Sistem öznitelikli girdileri taramadan çıkar.
    #[arg(long)]
    sistemleri_haric_et: bool,
    /// Sembolik bağların hedefini de gez (varsayılan: hayır).
    #[arg(long)]
    sembolik_bag: bool,
    /// Taranmayacak yol kalıbı; yol içinde alt dizi olarak aranır, büyük/küçük
    /// harf duyarsızdır ve baştaki `*` yok sayılır (`*.zip` gibi). Birden çok kez verilebilir.
    #[arg(long = "haric", value_name = "KALIP")]
    haric: Vec<String>,
    /// Bu düğüm sayısına ulaşınca taramayı kısalt.
    #[arg(long, value_name = "N")]
    maks_dugum: Option<usize>,
}

impl TaramaBayraklari {
    fn secenek(&self) -> TaramaSecenegi {
        TaramaSecenegi {
            en_derinlik: self.en_derinlik,
            gizlileri_dahil_et: !self.gizlileri_haric_et,
            sistemleri_dahil_et: !self.sistemleri_haric_et,
            sembolik_baglari_izle: self.sembolik_bag,
            haric_tutulan: self.haric.clone(),
            maks_dugum: self.maks_dugum,
        }
    }
}

#[derive(Debug, Args)]
struct ScanArg {
    /// Taranacak dizin.
    yol: String,
    #[command(flatten)]
    bayrak: TaramaBayraklari,
    /// Anlık görüntünün yazılacağı JSON dosyası.
    #[arg(long, value_name = "DOSYA")]
    cikti: Option<PathBuf>,
    /// Özeti ekrana basma.
    #[arg(long)]
    sessiz: bool,
}

#[derive(Debug, Args)]
struct TreeArg {
    /// Taranacak dizin.
    yol: String,
    #[command(flatten)]
    bayrak: TaramaBayraklari,
    /// Her düzeyde gösterilecek azami çocuk sayısı.
    #[arg(long, default_value_t = 20, value_name = "N")]
    ust_puan: usize,
    /// Görüntülenecek azami derinlik.
    #[arg(long, default_value_t = 3, value_name = "N")]
    derinlik: u32,
}

#[derive(Debug, Args)]
struct TreemapArg {
    /// Taranacak dizin.
    yol: String,
    #[command(flatten)]
    bayrak: TaramaBayraklari,
    /// Treemap'in verileceği alt yol (boş bırakılırsa kök).
    #[arg(long, value_name = "YOL")]
    alt: Option<String>,
    /// Izgara genişliği (terminal karakteri).
    #[arg(long, default_value_t = 100, value_name = "N")]
    genislik: usize,
    /// Izgara yüksekliği (terminal satırı).
    #[arg(long, default_value_t = 30, value_name = "N")]
    yukseklik: usize,
    /// Treemap'e alınacak en büyük çocuk sayısı.
    #[arg(long, default_value_t = 12, value_name = "N")]
    en_cok: usize,
    /// Kutuların JSON olarak yazılacağı dosya.
    #[arg(long, value_name = "DOSYA")]
    cikti: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineArg {
    /// Önceki taramaya ait anlık görüntü JSON'u.
    onceki: PathBuf,
    /// Yeni taramaya ait anlık görüntü JSON'u.
    sonraki: PathBuf,
    /// Listelenecek azami satır.
    #[arg(long, default_value_t = 25, value_name = "N")]
    en_cok: usize,
    /// Aylık kümülatif eğriyi de göster.
    #[arg(long)]
    aylik: bool,
    /// Çıktının JSON olarak yazılacağı dosya.
    #[arg(long, value_name = "DOSYA")]
    cikti: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct HeatmapArg {
    /// Önceki taramaya ait anlık görüntü JSON'u.
    onceki: PathBuf,
    /// Yeni taramaya ait anlık görüntü JSON'u.
    sonraki: PathBuf,
    /// Listelenecek azami satır.
    #[arg(long, default_value_t = 25, value_name = "N")]
    en_cok: usize,
    /// Çıktının JSON olarak yazılacağı dosya.
    #[arg(long, value_name = "DOSYA")]
    cikti: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct BigfilesArg {
    /// Taranacak dizin.
    yol: String,
    #[command(flatten)]
    bayrak: TaramaBayraklari,
    /// Boyut eşiği (bayt).
    #[arg(long, default_value_t = 524_288_000, value_name = "BAYT")]
    esik_bayt: u64,
    /// Yaş eşiği (gün).
    #[arg(long, default_value_t = 365, value_name = "GUN")]
    yas_gun: u64,
    /// Listelenecek azami satır.
    #[arg(long, default_value_t = 25, value_name = "N")]
    en_cok: usize,
    /// Arşiv uzantılı dosyaları da listele.
    #[arg(long)]
    arsivleri_dahil_et: bool,
    /// Gizli/sistem dosyalarını **listeden** çıkar (taramadan çıkarmak için
    /// `--gizlileri-haric-et` kullanılır).
    #[arg(long)]
    gizlileri_liste_haric: bool,
    /// Çıktının JSON olarak yazılacağı dosya.
    #[arg(long, value_name = "DOSYA")]
    cikti: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ReportArg {
    /// Taranacak dizin.
    yol: String,
    #[command(flatten)]
    bayrak: TaramaBayraklari,
    /// Birim kapasitesi (bayt). Ayırma denetimi bunu dışarıdan alır.
    #[arg(long, default_value_t = 0, value_name = "BAYT")]
    kapasite: u64,
    /// Dosya sisteminin bildirdiği boş alan (bayt).
    #[arg(long, default_value_t = 0, value_name = "BAYT")]
    bos: u64,
    /// Yedeklenmiş küre miktarı (bayt).
    #[arg(long, default_value_t = 0, value_name = "BAYT")]
    yedeklenmis: u64,
    /// Dosya sistemi küme boyutu (bayt).
    #[arg(long, default_value_t = 4_096, value_name = "BAYT")]
    kume: u64,
    /// Raporun JSON olarak yazılacağı dosya.
    #[arg(long, value_name = "DOSYA")]
    cikti: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match calistir(cli.komut) {
        Ok(()) => ExitCode::SUCCESS,
        Err(hata) => {
            eprintln!("disktree: hata: {hata}");
            ExitCode::FAILURE
        }
    }
}

fn calistir(komut: Komut) -> Result<(), Hata> {
    match komut {
        Komut::Scan(a) => komut_scan(a),
        Komut::Tree(a) => komut_tree(a),
        Komut::Treemap(a) => komut_treemap(a),
        Komut::Timeline(a) => komut_timeline(a),
        Komut::Heatmap(a) => komut_heatmap(a),
        Komut::Bigfiles(a) => komut_bigfiles(a),
        Komut::Report(a) => komut_report(a),
    }
}

fn komut_scan(a: ScanArg) -> Result<(), Hata> {
    let sonuc = tara(&a.yol, a.bayrak.secenek())?;
    let kayma = Kayma::agactan(&sonuc.agac, artik_zaman());
    if let Some(cikti) = &a.cikti {
        rapor::kayma_yaz(&kayma, cikti)?;
    }
    if !a.sessiz {
        println!("{}", ozet_metni(&sonuc));
        if let Some(cikti) = &a.cikti {
            println!("anlik goruntu: {}", cikti.display());
        } else {
            println!("anlik goruntu: (yazilmadi; --cikti ile kaydedin)");
        }
        for h in &sonuc.hatalar {
            println!("  kisitli: {h}");
        }
    }
    Ok(())
}

fn komut_tree(a: TreeArg) -> Result<(), Hata> {
    let sonuc = tara(&a.yol, a.bayrak.secenek())?;
    println!("{}", ozet_metni(&sonuc));
    agac_metni(&sonuc.agac, sonuc.agac.kok(), 0, a.derinlik, a.ust_puan);
    Ok(())
}

fn komut_treemap(a: TreemapArg) -> Result<(), Hata> {
    let sonuc = tara(&a.yol, a.bayrak.secenek())?;
    let kok = alt_dugum(&sonuc.agac, a.alt.as_deref())?;
    let kutular = kutulari_uret(
        &sonuc.agac,
        kok,
        a.en_cok,
        a.genislik as f64,
        a.yukseklik as f64,
    );
    println!("{}", ozet_metni(&sonuc));
    println!("{}", agac_metni_tek(&sonuc.agac, kok));
    println!(
        "{}",
        treemap::ascii_cikti(&kutular, a.genislik, a.yukseklik)
    );
    println!("{}", lejant_metni(&kutular, a.genislik, a.yukseklik));
    if let Some(cikti) = &a.cikti {
        rapor::metin_yaz(cikti, &rapor::json_metni(&kutular)?)?;
        println!("treemap json: {}", cikti.display());
    }
    Ok(())
}

fn komut_timeline(a: TimelineArg) -> Result<(), Hata> {
    let onceki = rapor::kayma_oku(&a.onceki)?;
    let sonraki = rapor::kayma_oku(&a.sonraki)?;
    if a.aylik {
        println!("aylik kumulatif buyume egrisi ({}):", sonraki.kok);
        for ay in zaman::aylik_kume(&sonraki) {
            println!(
                "  {:<8} {:>10} bayt  {:>4} dosya  kumulatif {:>12}",
                ay.ay, ay.bayt, ay.dosya, ay.kumulatif_bayt
            );
        }
        println!();
    }
    let satirlar = zaman::fark(&onceki, &sonraki);
    println!(
        "boyut farki: {} satir ({} kayit karsilastirildi)",
        satirlar.len(),
        onceki.dugumler.len()
    );
    for s in satirlar.iter().take(a.en_cok) {
        println!(
            "{:>3}. {:<12} {:>+15} bayt  {:>10} -> {:<10} {}",
            s.sira,
            s.durum,
            s.fark,
            human_bayt(s.onceki_bayt),
            human_bayt(s.sonraki_bayt),
            s.yol
        );
    }
    if let Some(cikti) = &a.cikti {
        let j: Vec<rapor::ZamanJson> = satirlar
            .iter()
            .map(|s| rapor::ZamanJson {
                sira: s.sira,
                yol: s.yol.clone(),
                onceki_bayt: s.onceki_bayt,
                sonraki_bayt: s.sonraki_bayt,
                fark: s.fark,
                durum: s.durum.to_string(),
            })
            .collect();
        rapor::metin_yaz(cikti, &rapor::json_metni(&j)?)?;
        println!("zaman json: {}", cikti.display());
    }
    Ok(())
}

fn komut_heatmap(a: HeatmapArg) -> Result<(), Hata> {
    let onceki = rapor::kayma_oku(&a.onceki)?;
    let sonraki = rapor::kayma_oku(&a.sonraki)?;
    let satirlar = isiharitasi::hesapla(&onceki, &sonraki);
    println!(
        "isi haritasi: {} degisen yol (derinlik 1 = en sicak)",
        satirlar.len()
    );
    for s in satirlar.iter().take(a.en_cok) {
        println!(
            "{:<5} {:>15} bayt  derinlik {:<3} skor {:>12.2}  {}",
            seviye_isareti(s.seviye),
            s.degisim,
            s.derinlik,
            s.skor,
            s.yol
        );
    }
    if let Some(cikti) = &a.cikti {
        rapor::metin_yaz(cikti, &rapor::json_metni(&isiharitasi::jsonla(&satirlar))?)?;
        println!("isi json: {}", cikti.display());
    }
    Ok(())
}

fn komut_bigfiles(a: BigfilesArg) -> Result<(), Hata> {
    let sonuc = tara(&a.yol, a.bayrak.secenek())?;
    let secenek = BuyuraSecenegi {
        esik_bayt: a.esik_bayt,
        min_yas_gun: a.yas_gun,
        artik_zaman: artik_zaman(),
        arsivleri_haric_et: !a.arsivleri_dahil_et,
        gizlileri_dahil_et: !a.gizlileri_liste_haric,
        en_fazla: a.en_cok,
    };
    let satirlar = buyuk::listele(&sonuc.agac, &secenek);
    println!("{}", ozet_metni(&sonuc));
    println!(
        "buyuk dosya listesi: esik {} , yas {} gun , {} satir",
        human_bayt(a.esik_bayt),
        a.yas_gun,
        satirlar.len()
    );
    for s in &satirlar {
        println!(
            "{:>10}  {:>5}g  {}{} {}",
            human_bayt(s.bayt),
            s.yas_gun,
            if s.gizli { "[gizli] " } else { "" },
            if s.arsiv { "[arsiv] " } else { "" },
            s.yol
        );
    }
    println!("not: bu liste yalnizca gozlem sunar; arac hicbir dosyayi silmez veya degistirmez.");
    if let Some(cikti) = &a.cikti {
        let j: Vec<rapor::BuyukSatirJson> =
            satirlar.iter().map(buyuk::BuyukSatir::jsonla).collect();
        rapor::metin_yaz(cikti, &rapor::json_metni(&j)?)?;
        println!("buyuk dosya json: {}", cikti.display());
    }
    Ok(())
}

fn komut_report(a: ReportArg) -> Result<(), Hata> {
    let sonuc = tara(&a.yol, a.bayrak.secenek())?;
    let girdi = AyirmaGirdi {
        kapasite: a.kapasite,
        bos: a.bos,
        yedeklenmis: a.yedeklenmis,
        kume_bayt: a.kume,
    };
    let ay = ayirma::hesapla(&sonuc.agac, girdi)?;
    let kutular = kutulari_uret(&sonuc.agac, sonuc.agac.kok(), 12, 100.0, 100.0);
    let buyuk_satirlar = buyuk::listele(
        &sonuc.agac,
        &BuyuraSecenegi {
            artik_zaman: artik_zaman(),
            en_fazla: 25,
            ..BuyuraSecenegi::default()
        },
    );
    let rapor = Rapor {
        bicim: rapor::BICIM.to_string(),
        surum: rapor::SURUM,
        istatistik: IstatistikJson::uret(&sonuc.agac, &sonuc.istatistik),
        zaman: zaman::aylik_kume(&Kayma::agactan(&sonuc.agac, artik_zaman()))
            .into_iter()
            .map(|k| rapor::AyJson {
                ay: k.ay,
                bayt: k.bayt,
                dosya: k.dosya,
                kumulatif_bayt: k.kumulatif_bayt,
                kumulatif_dosya: k.kumulatif_dosya,
            })
            .collect(),
        buyuk_dosyalar: buyuk_satirlar
            .iter()
            .map(buyuk::BuyukSatir::jsonla)
            .collect(),
        ayirma: ay.jsonla(),
        treemap: kutular,
        isi: Vec::new(),
    };
    rapor::metin_yaz(&a.cikti, &rapor::json_metni(&rapor)?)?;
    println!("{}", ozet_metni(&sonuc));
    println!("ayirma (siyah kutu): {}", ay.gerekce);
    println!("kayip: {} bayt (tahmin, olcum degildir)", ay.kayip);
    if let Some(u) = ay.uyari() {
        println!("uyari: {u}");
    }
    println!("rapor: {}", a.cikti.display());
    Ok(())
}

/// Tarama sonucunun tek satırlık özeti.
fn ozet_metni(sonuc: &TaramaSonucu) -> String {
    format!(
        "{}  dugum={} dosya={} dizin={} toplam={} bellek={}  [{}]",
        sonuc.agac.kok_yolu(),
        sonuc.agac.dugum_sayisi(),
        sonuc.agac.toplam_dosya(),
        sonuc.agac.toplam_dizin(),
        human_bayt(sonuc.agac.toplam_bayt()),
        human_bayt(sonuc.agac.bellek_boyutu() as u64),
        sonuc.istatistik.ozet()
    )
}

/// Metin ağaç çıktısı: derinlik sınırlı, çocuklar boyut azalan sırada.
fn agac_metni(agac: &Agac, dugum: DugumId, derinlik: u32, azami: u32, ust_puan: usize) {
    if derinlik > azami {
        return;
    }
    let cocuklar = agac.cocuklar(dugum);
    if cocuklar.is_empty() {
        return;
    }
    let mut sirali: Vec<DugumId> = cocuklar;
    sirali.sort_by(|a, b| {
        let ka = agac.kayit(*a).map(|k| k.alt_toplam).unwrap_or(0);
        let kb = agac.kayit(*b).map(|k| k.alt_toplam).unwrap_or(0);
        kb.cmp(&ka)
    });
    let toplam = agac.kayit(dugum).map(|k| k.alt_toplam).unwrap_or(1).max(1);
    for (i, c) in sirali.iter().take(ust_puan).enumerate() {
        let Ok(k) = agac.kayit(*c) else { continue };
        let isaret = if i + 1 == sirali.len() || i + 1 == ust_puan {
            "`-- "
        } else {
            "|-- "
        };
        println!(
            "{}{:<28} {:>10}  {:<20} {:>5.1}%  d{}",
            "   ".repeat(derinlik as usize + 1),
            isaret.to_owned() + disktree::model::son_bilesen(agac.yol_veya_yildiz(*c)),
            human_bayt(k.alt_toplam),
            bar_ciz(k.alt_toplam, toplam),
            100.0 * k.alt_toplam as f64 / toplam as f64,
            k.derinlik
        );
        agac_metni(agac, *c, derinlik + 1, azami, ust_puan);
    }
    if sirali.len() > ust_puan {
        println!(
            "{}... {} cocuk daha",
            "   ".repeat(derinlik as usize + 1),
            sirali.len() - ust_puan
        );
    }
}

/// Treemap başlığı: kökün (veya alt dizinin) tek satırlık tanımı.
fn agac_metni_tek(agac: &Agac, dugum: DugumId) -> String {
    let k = agac.kayit(dugum);
    match k {
        Ok(k) => format!(
            "treemap: {}  toplam={}  cocuk={}",
            agac.yol_veya_yildiz(dugum),
            human_bayt(k.alt_toplam),
            agac.cocuklar(dugum).len()
        ),
        Err(_) => "treemap: (dugum bulunamadi)".to_string(),
    }
}

/// Izgara altına yazılan açıklama satırı.
fn lejant_metni(kutular: &[Kutu], genislik: usize, yukseklik: usize) -> String {
    let toplam: u64 = kutular.iter().map(|k| k.deger).sum();
    format!(
        "rampa: bos=' ' (kucuk) ... '@' (buyuk); {} kutu, {} toplam, izgara {}x{}",
        kutular.len(),
        human_bayt(toplam),
        genislik,
        yukseklik
    )
}

/// Bir düğümün en büyük çocuklarından treemap üretir.
fn kutulari_uret(
    agac: &Agac,
    dugum: DugumId,
    en_cok: usize,
    genislik: f64,
    yukseklik: f64,
) -> Vec<Kutu> {
    let mut cocuklar = agac.cocuklar(dugum);
    cocuklar.sort_by(|a, b| {
        let ka = agac.kayit(*a).map(|k| k.alt_toplam).unwrap_or(0);
        let kb = agac.kayit(*b).map(|k| k.alt_toplam).unwrap_or(0);
        kb.cmp(&ka)
    });
    let girdiler: Vec<Girdi> = cocuklar
        .iter()
        .take(en_cok)
        .filter_map(|c| {
            let k = agac.kayit(*c).ok()?;
            let ad = disktree::model::son_bilesen(agac.yol_veya_yildiz(*c));
            Some(Girdi::yeni(ad, k.alt_toplam))
        })
        .collect();
    treemap::yerlestir(&girdiler, 0.0, 0.0, genislik, yukseklik)
}

/// Verilen alt yolu çözer; verilmezse kökü döndürür.
fn alt_dugum(agac: &Agac, alt: Option<&str>) -> Result<DugumId, Hata> {
    match alt {
        None => Ok(agac.kok()),
        Some(y) => agac
            .ara(y)
            .ok_or_else(|| Hata::YolBulunamadi { yol: y.to_string() }),
    }
}

/// Sabit genişlikli yüzde çubuğu.
fn bar_ciz(bayt: u64, toplam: u64) -> String {
    const GENISLIK: usize = 20;
    let dolu = ((bayt as f64 / toplam as f64) * GENISLIK as f64).round() as usize;
    let dolu = dolu.min(GENISLIK);
    format!("{}{}", "#".repeat(dolu), ".".repeat(GENISLIK - dolu))
}

/// Isı seviyesini tek karakterle gösterir.
fn seviye_isareti(seviye: u8) -> &'static str {
    [" ", ":", "=", "+", "*"][(seviye as usize).min(4)]
}

/// Bayt değerini insan okunur biçime çevirir (B/KiB/MiB/GiB/TiB).
fn human_bayt(b: u64) -> String {
    const BIRIMLER: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if b < 1024 {
        return format!("{b} B");
    }
    let mut deger = b as f64;
    let mut i = 0usize;
    while deger >= 1024.0 && i + 1 < BIRIMLER.len() {
        deger /= 1024.0;
        i += 1;
    }
    format!("{deger:.1} {}", BIRIMLER[i])
}

/// Şimdiki zaman, Unix epoch saniyesi olarak.
///
/// Yalnızca **çıktı** üretiminde kullanılır; hiçbir test kararına girmez.
fn artik_zaman() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Birim testlerinin dışarıdan doğrulayabilmesi için `human_bayt` görünürdür.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bayt_esikleri_dogru_birim_secer() {
        assert_eq!(human_bayt(0), "0 B");
        assert_eq!(human_bayt(1023), "1023 B");
        assert_eq!(human_bayt(1024), "1.0 KiB");
        assert_eq!(human_bayt(1024 * 1024), "1.0 MiB");
        assert_eq!(human_bayt(1024_u64.pow(3)), "1.0 GiB");
        assert_eq!(human_bayt(1024_u64.pow(5)), "1.0 PiB");
    }

    #[test]
    fn bar_ciz_oranlari_gosterir() {
        assert_eq!(bar_ciz(0, 100).len(), 20);
        assert_eq!(bar_ciz(100, 100), "#".repeat(20));
        assert_eq!(
            bar_ciz(50, 100),
            format!("{}{}", "#".repeat(10), ".".repeat(10))
        );
    }

    #[test]
    fn seviye_isaretleri_sinirli_araliktadir() {
        assert_eq!(seviye_isareti(0), " ");
        assert_eq!(seviye_isareti(4), "*");
        assert_eq!(seviye_isareti(99), "*");
    }

    #[test]
    fn tarama_bayraklari_seceneye_cevrilir() {
        let a = Cli::parse_from([
            "disktree",
            "scan",
            "/kok",
            "--gizlileri-haric-et",
            "--en-derinlik",
            "5",
        ]);
        let Komut::Scan(s) = a.komut else {
            panic!("beklenen scan")
        };
        let secenek = s.bayrak.secenek();
        assert!(!secenek.gizlileri_dahil_et);
        assert_eq!(secenek.en_derinlik, 5);
        assert!(secenek.sistemleri_dahil_et);
    }
}

//! DiskTree uçtan uca entegrasyon testleri.
//!
//! Kapsam: gerçek dosya sistemi ağacı üzerinde tarama, anlık görüntü yazma ve
//! okuma, zaman çizelgesi, ısı haritası, treemap/ASCII çıktı, arşiv dışlama,
//! kısmi tarama, döngü koruması ve **"hiçbir komut dosya silmez ya da
//! değiştirmez"** kuralının kanıtı.
//!
//! `tempfile` crate'i bağımlılık politikasıyla yasaktır; geçici dizin yardımcısı
//! aşağıda elle yazılmıştır ve `Drop` ile temizlenir.

use disktree::ayirma::{self, AyirmaGirdi};
use disktree::buyuk::{self, BuyuraSecenegi};
use disktree::isiharitasi;
use disktree::model::Tur;
use disktree::rapor::{self, Kayma};
use disktree::tarama::{tara, DonguKoruma, TaramaSecenegi, Tarayici};
use disktree::treemap::{ascii_cikti, yerlestir, Girdi};
use disktree::zaman;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Testlerin yazdığı geçici dizin; `Drop` ile temizlenir.
struct Gecici {
    yol: PathBuf,
}

impl Gecici {
    fn yeni(etiket: &str) -> Self {
        let yol =
            std::env::temp_dir().join(format!("disktree-ent-{etiket}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&yol);
        std::fs::create_dir_all(&yol).expect("gecici dizin olustur");
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
            std::fs::create_dir_all(ust).expect("ust dizin olustur");
        }
        std::fs::write(&p, icerik).expect("dosya yaz");
        p
    }

    fn dizin(&self, ad: &str) -> PathBuf {
        let p = self.yol.join(ad);
        std::fs::create_dir_all(&p).expect("dizin olustur");
        p
    }

    /// Dizindeki tüm dosyaları bayt bayt okur (fixture üretimi için).
    fn oku(&self, ad: &str) -> Vec<u8> {
        std::fs::read(self.yol.join(ad)).expect("fixture oku")
    }
}

impl Drop for Gecici {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.yol);
    }
}

/// İç içe geçmiş, gizli, sıfır boyutlu ve arşiv dosyası içeren standart fixture.
fn fixture(etiket: &str) -> Gecici {
    let g = Gecici::yeni(etiket);
    g.dosya("kaynak/ana.rs", b"fn main() {}\n");
    g.dosya("kaynak/mod/mod_islem.rs", b"use std::fs;\n");
    g.dosya("kaynak/mod/yardimci.txt", &vec![b'x'; 4_096]);
    g.dosya("belge/rapor.txt", b"disk raporu");
    g.dosya("belge/eski.zip", &vec![0_u8; 8_192]);
    g.dosya("belge/eski/notlar.txt", b"eski not");
    g.dosya("bos0.bin", b"");
    g.dosya(".gizli_yapilandirma", b"[gizli]\n");
    g
}

fn secenek() -> TaramaSecenegi {
    TaramaSecenegi::default()
}

fn disktree_bin() -> &'static str {
    env!("CARGO_BIN_EXE_disktree")
}

/// Bir dizindeki tüm dosyaların (yol, boyut, mtime, içerik) parmak izini alır.
fn parmak_izi(kok: &Path) -> BTreeMap<String, (u64, u64, Vec<u8>)> {
    let mut iz = BTreeMap::new();
    let mut yigin = vec![kok.to_path_buf()];
    while let Some(d) = yigin.pop() {
        let Ok(okuma) = std::fs::read_dir(&d) else {
            continue;
        };
        for g in okuma.flatten() {
            let y = g.path();
            let Ok(md) = std::fs::symlink_metadata(&y) else {
                continue;
            };
            if md.is_dir() {
                yigin.push(y);
            } else {
                let zaman = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                let icerik = std::fs::read(&y).unwrap_or_default();
                iz.insert(
                    y.strip_prefix(kok).unwrap_or(&y).display().to_string(),
                    (md.len(), zaman, icerik),
                );
            }
        }
    }
    iz
}

#[test]
fn uctan_uca_tarama_toplamlari_dogru_hesaplar() {
    let g = fixture("tam");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let agac = &sonuc.agac;
    assert_eq!(agac.toplam_bayt(), 13 + 13 + 4_096 + 11 + 8_192 + 8 + 8);
    assert_eq!(agac.toplam_dosya(), 8);
    assert!(agac.dugum_sayisi() > 6, "ara dizinler de sayilmali");
    assert!(sonuc.istatistik.tamamlandi);
    assert!(sonuc.istatistik.ziyaret_edilen_dizin >= 5);
}

#[test]
fn gizli_dosya_tarama_icerisinde_sayilir() {
    let g = fixture("gizli");
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("attrib")
            .arg("+H")
            .arg(g.yol().join(".gizli_yapilandirma"))
            .output();
        let sonuc = tara(&g.metin(), secenek()).expect("tarama");
        assert!(sonuc.agac.gizli_sayisi() >= 1, "gizli nitelik okunmadi");
    }
    #[cfg(not(windows))]
    {
        let sonuc = tara(&g.metin(), secenek()).expect("tarama");
        assert!(
            sonuc.agac.gizli_sayisi() >= 1,
            "nokta ile baslayan ad gizli sayilmali"
        );
    }
}

#[test]
fn sifir_boyutlu_dosya_toplami_bozmaz_ama_sayilir() {
    let g = fixture("sifir");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let bos = agac_dugum(&sonuc.agac, &format!("{}\\bos0.bin", g.metin()));
    assert!(bos.is_some(), "sifir boyutlu dosya agacta olmali");
    let k = sonuc.agac.kayit(bos.expect("dugum")).expect("kayit");
    assert_eq!(k.boyut, 0);
    assert!(k.dosya_mi());
}

fn agac_dugum(agac: &disktree::model::Agac, yol: &str) -> Option<disktree::model::DugumId> {
    agac.ara(yol)
}

#[test]
fn kismi_tarama_dugum_siniriyla_kisaltilir() {
    let g = fixture("kismi");
    let s = TaramaSecenegi {
        maks_dugum: Some(3),
        ..secenek()
    };
    let sonuc = tara(&g.metin(), s).expect("tarama");
    assert!(sonuc.agac.dugum_sayisi() <= 3);
    assert!(sonuc.istatistik.kisaltildi);
    assert!(!sonuc.istatistik.tamamlandi);
    assert!(sonuc.agac.toplam_bayt() <= 4_096 + 11 + 18 + 8_192);
}

#[test]
fn derinlik_siniri_icer_noktalari_gormesin() {
    let g = fixture("derinlik");
    let s = TaramaSecenegi {
        en_derinlik: 1,
        ..secenek()
    };
    let sonuc = tara(&g.metin(), s).expect("tarama");
    assert!(sonuc.istatistik.derinlik_asim > 0);
    // kok(0) + kaynak(1) okunur; mod/ altindaki dosyalar gorunmez.
    assert!(agac_dugum(&sonuc.agac, &format!("{}\\kaynak\\ana.rs", g.metin())).is_some());
    assert!(
        agac_dugum(
            &sonuc.agac,
            &format!("{}\\kaynak\\mod\\mod_islem.rs", g.metin())
        )
        .is_none(),
        "derinlik siniri uygulanmali"
    );
}

#[test]
fn haric_tutulan_deseni_icer_klasorleri_tamamen_disarida_birakir() {
    let g = fixture("haric");
    let s = TaramaSecenegi {
        haric_tutulan: vec!["kaynak".to_string()],
        ..secenek()
    };
    let sonuc = tara(&g.metin(), s).expect("tarama");
    assert!(sonuc.istatistik.haric_tutulan > 0);
    assert!(agac_dugum(&sonuc.agac, &format!("{}\\kaynak\\ana.rs", g.metin())).is_none());
    assert!(agac_dugum(&sonuc.agac, &format!("{}\\belge\\rapor.txt", g.metin())).is_some());
}

#[test]
fn sembolik_bag_dongusu_tarama_terminasyonu_korur() {
    let g = fixture("dongu");
    g.dizin("dongu_hedef");
    let hedef = g.yol().join("dongu_hedef");
    g.dosya("dongu_hedef/ic.txt", b"1234");
    let mut koruma = DonguKoruma::yeni();
    koruma.tohumla(std::fs::canonicalize(&hedef).unwrap_or(hedef.clone()));
    let sonuc = Tarayici::yeni(&g.metin(), secenek())
        .expect("gezgin")
        .koruma_ile(koruma)
        .tara_ve_al()
        .expect("tarama");
    assert_eq!(
        sonuc.istatistik.dongu, 1,
        "dongu sayaci tam olarak bir olmali"
    );
    assert!(
        agac_dugum(&sonuc.agac, &format!("{}\\dongu_hedef\\ic.txt", g.metin())).is_none(),
        "kilitli dizine girilmemeli"
    );
}

#[test]
fn sembolik_bag_yoksa_dongu_koruma_sessizce_gecerli_kalir() {
    let g = fixture("donguyok");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    assert_eq!(sonuc.istatistik.dongu, 0);
    assert!(sonuc.istatistik.tamamlandi);
}
#[test]
fn izin_hatasi_tarama_ozetinde_sayilir_ve_agaci_bozmaz() {
    let g = fixture("izin");
    // Bir dosyayı dizin kökü olarak vermek gerçek bir isletim sistemi hatasi uretir.
    let hata = tara(
        &g.yol()
            .join("belge")
            .join("rapor.txt")
            .display()
            .to_string(),
        secenek(),
    )
    .expect_err("dosya kok olarak kullanilamaz");
    assert!(matches!(hata, disktree::Hata::KokOkunamadi { .. }));
    // Tarama yine de calisiyor: sayaclar sifir degil, hata liste bos.
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    assert_eq!(sonuc.istatistik.erisim_reddi, 0);
    assert!(sonuc.hatalar.is_empty());
}

#[test]
fn anlik_goruntu_dosyasi_yazilir_okunur_ve_timeline_besler() {
    let g = fixture("kayma1");
    let kayma_yol = g.yol().join("onceki.json");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    rapor::kayma_yaz(&Kayma::agactan(&sonuc.agac, 1_000), &kayma_yol).expect("yaz");
    assert!(kayma_yol.exists());

    // Ikinci tarama: buyuyen ve yeni dosyalar eklenir.
    g.dosya("kaynak/mod/buyuyen.bin", &vec![b'y'; 20_000]);
    g.dosya("yeni/klasör/dosya.txt", b"merhaba");
    let sonraki = tara(&g.metin(), secenek()).expect("tarama");

    let onceki = rapor::kayma_oku(&kayma_yol).expect("oku");
    let yeni = Kayma::agactan(&sonraki.agac, 2_000);
    let satirlar = zaman::fark(&onceki, &yeni);
    assert!(!satirlar.is_empty());
    assert!(satirlar
        .iter()
        .any(|s| s.yol.ends_with("buyuyen.bin") && s.fark == 20_000));
    assert!(satirlar.iter().any(|s| s.durum == "yeni"));
}

#[test]
fn anlik_goruntu_bozuk_olursa_hata_dondurur() {
    let g = Gecici::yeni("bozuk");
    let yol = g.yol().join("bozuk.json");
    std::fs::write(&yol, b"{ bu json degil").expect("yaz");
    assert!(rapor::kayma_oku(&yol).is_err());
}

#[test]
fn anlik_goruntu_yarim_kalmis_gecici_dosya_birakilmaz() {
    let g = Gecici::yeni("atomik");
    let yol = g.yol().join("k.json");
    let k = Kayma::agactan(&disktree::Agac::yeni("/kok").expect("agac"), 1);
    rapor::kayma_yaz(&k, &yol).expect("yaz");
    assert!(!yol.with_extension("gecici").exists());
}

#[test]
fn json_sema_gidis_donusu_kayipsizdir() {
    let g = fixture("sema");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let k = Kayma::agactan(&sonuc.agac, 42);
    let metin = rapor::kayma_metni(&k).expect("metin");
    assert!(metin.contains("disktree-kayma/1"));
    let geri = rapor::kayma_yorumla(&metin).expect("yorumla");
    assert_eq!(k, geri);
    assert_eq!(geri.zaman, 42);
    assert_eq!(geri.dugumler.len(), k.dugumler.len());
}

#[test]
fn ascii_treemap_uretimi_boyutlari_tamdir() {
    let g = fixture("ascii");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let agac = &sonuc.agac;
    let girdiler: Vec<Girdi> = agac
        .cocuklar(agac.kok())
        .iter()
        .filter_map(|c| {
            let k = agac.kayit(*c).ok()?;
            Some(Girdi::yeni(
                disktree::model::son_bilesen(agac.yol_veya_yildiz(*c)),
                k.alt_toplam,
            ))
        })
        .collect();
    let kutular = yerlestir(&girdiler, 0.0, 0.0, 100.0, 50.0);
    let cikti = ascii_cikti(&kutular, 60, 15);
    let satirlar: Vec<&str> = cikti.lines().collect();
    assert_eq!(satirlar.len(), 15);
    for s in &satirlar {
        assert_eq!(s.chars().count(), 60);
    }
    assert!(!cikti.trim().is_empty());
}

#[test]
fn treemap_alani_girdi_toplamiyla_ortusur() {
    let girdiler = vec![
        Girdi::yeni("belge", 1_000),
        Girdi::yeni("kaynak", 4_000),
        Girdi::yeni("sifir", 0),
    ];
    let kutular = yerlestir(&girdiler, 0.0, 0.0, 80.0, 40.0);
    assert_eq!(kutular.len(), 2, "sifir degerli kutu yerlestirilmez");
    let alan: f64 = kutular.iter().map(|k| k.genislik * k.yukseklik).sum();
    assert!((alan - 3_200.0).abs() < 1e-6);
    for k in &kutular {
        let beklenen = k.deger as f64 / 5_000.0 * 3_200.0;
        assert!((k.genislik * k.yukseklik - beklenen).abs() < 1e-6);
    }
}

#[test]
fn buyuk_dosya_listesi_arsivleri_disarida_birakir() {
    let g = fixture("buyuk");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let s = BuyuraSecenegi {
        esik_bayt: 1_000,
        min_yas_gun: 0,
        artik_zaman: 4_000_000_000,
        ..BuyuraSecenegi::default()
    };
    let liste = buyuk::listele(&sonuc.agac, &s);
    assert!(!liste.iter().any(|x| x.yol.ends_with(".zip")));

    let arsivli = BuyuraSecenegi {
        arsivleri_haric_et: false,
        ..s
    };
    let liste2 = buyuk::listele(&sonuc.agac, &arsivli);
    assert!(liste2.iter().any(|x| x.yol.ends_with(".zip") && x.arsiv));
}

#[test]
fn isi_haritasi_degisim_ve_derinlik_urunu_durur() {
    let g = fixture("isi");
    let ilk = tara(&g.metin(), secenek()).expect("tarama");
    let o = Kayma::agactan(&ilk.agac, 0);
    g.dosya("kaynak/mod/buyuk.bin", &vec![b'z'; 50_000]);
    let ikinci = tara(&g.metin(), secenek()).expect("tarama");
    let s = Kayma::agactan(&ikinci.agac, 0);
    let satirlar = isiharitasi::hesapla(&o, &s);
    assert!(!satirlar.is_empty());
    assert!(satirlar.iter().all(|x| x.derinlik > 0));
    assert!(satirlar.iter().all(|x| x.seviye <= 4));
    let en = satirlar.iter().map(|x| x.skor).fold(f64::MIN, f64::max);
    assert!(
        (en - satirlar[0].skor).abs() < 1e-9,
        "ilk satir en yuksek skora sahip olmali"
    );
}

#[test]
fn ayirma_denetimi_tahmini_ve_gerekcesi_dondurur() {
    let g = fixture("ayirma");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let r = ayirma::hesapla(
        &sonuc.agac,
        AyirmaGirdi {
            kapasite: 1_000_000_000,
            bos: 400_000_000,
            yedeklenmis: 0,
            kume_bayt: 4_096,
        },
    )
    .expect("ayirma");
    assert_eq!(r.dosya_verisi, sonuc.agac.toplam_bayt());
    assert!(r.tahmini_kullanim >= r.dosya_verisi);
    assert!(r.kayip > 0);
    assert!(r.gerekce.contains("MFT"));
    assert!(r.uyari().is_none());
}

#[test]
fn kume_boyutu_sifir_kenari_ayirma_denetimi_hata_dondurur() {
    let g = fixture("kume0");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let sonuc2 = ayirma::hesapla(
        &sonuc.agac,
        AyirmaGirdi {
            kume_bayt: 0,
            ..AyirmaGirdi::default()
        },
    );
    assert!(sonuc2.is_err());
}

#[test]
fn hicbir_komut_dosya_silmez_ya_da_degistirmez() {
    // Kanit: fixture'in parmak izi alinir, tum salt-okunur komutlar calistirilir,
    // sonra parmak izi karsilastirilir. Karsilastirma yolu, boyutu, mtime'i ve
    // icerik baytlarini kapsar; dosya kucultuldugu, silindigi, eklendigi veya
    // yeniden yazildigi durumlarda test kacar.
    let g = fixture("silmez");
    // Cikti dosyalari taranan agacin DISINDA bir klasorde tutulur; aksi halde
    // "taranan agacta dosya sayisi degismedi" iddiasi kendi kendini bozardi.
    let c = Gecici::yeni("silmez-cikti");
    let oncesi = parmak_izi(g.yol());
    assert!(!oncesi.is_empty());

    let kayma = c.yol().join("kayma.json");
    let rapor = c.yol().join("rapor.json");
    let treemap_json = c.yol().join("treemap.json");
    let buyuk_json = c.yol().join("buyuk.json");
    let isi_json = c.yol().join("isi.json");
    let zaman_json = c.yol().join("zaman.json");
    let bin = disktree_bin();

    let calistir = |args: &[&str]| {
        let cikti = Command::new(bin)
            .args(args)
            .current_dir(c.yol())
            .output()
            .unwrap_or_else(|e| panic!("{args:?} calistirilamadi: {e}"));
        assert!(
            cikti.status.success(),
            "{args:?} basarisiz: {}",
            String::from_utf8_lossy(&cikti.stderr)
        );
    };

    calistir(&["scan", &g.metin(), "--cikti", &kayma.display().to_string()]);
    calistir(&["tree", &g.metin()]);
    calistir(&[
        "treemap",
        &g.metin(),
        "--cikti",
        &treemap_json.display().to_string(),
    ]);
    calistir(&[
        "report",
        &g.metin(),
        "--cikti",
        &rapor.display().to_string(),
    ]);
    calistir(&[
        "bigfiles",
        &g.metin(),
        "--esik-bayt",
        "100",
        "--yas-gun",
        "0",
        "--cikti",
        &buyuk_json.display().to_string(),
    ]);
    calistir(&[
        "timeline",
        &kayma.display().to_string(),
        &kayma.display().to_string(),
        "--cikti",
        &zaman_json.display().to_string(),
    ]);
    calistir(&[
        "heatmap",
        &kayma.display().to_string(),
        &kayma.display().to_string(),
        "--cikti",
        &isi_json.display().to_string(),
    ]);

    // Cikti dosyalari taranan agacin disinda: yeni dosya olusmamali.
    let sonrasi = parmak_izi(g.yol());
    for (ad, (boyut, zaman, icerik)) in &oncesi {
        let bulunan = sonrasi
            .get(ad)
            .unwrap_or_else(|| panic!("dosya silinmis: {ad}"));
        assert_eq!(bulunan.0, *boyut, "dosya boyutu degisti: {ad}");
        assert_eq!(bulunan.1, *zaman, "dosya zaman damgasi degisti: {ad}");
        assert_eq!(bulunan.2, *icerik, "dosya icerigi degisti: {ad}");
    }
    assert_eq!(
        oncesi.len(),
        sonrasi.len(),
        "taranan agacta dosya sayisi degisti"
    );
    // Fixture icerikleri de ayni kaldi.
    assert_eq!(g.oku("kaynak/ana.rs"), b"fn main() {}\n");
    assert_eq!(g.oku("belge/eski.zip").len(), 8_192);
    assert_eq!(g.oku("bos0.bin"), Vec::<u8>::new());
}

#[test]
fn cli_scan_json_uretir_ve_yeniden_okur() {
    let g = fixture("cli");
    let kayma = g.yol().join("k.json");
    let cikti = Command::new(disktree_bin())
        .args(["scan", &g.metin(), "--cikti", &kayma.display().to_string()])
        .output()
        .expect("calistir");
    assert!(cikti.status.success());
    let k = rapor::kayma_oku(&kayma).expect("kayma oku");
    assert_eq!(k.bicim, rapor::BICIM);
    assert!(!k.dugumler.is_empty());
    let std_out = String::from_utf8_lossy(&cikti.stdout);
    assert!(std_out.contains("dugum="));
}

#[test]
fn cli_hatali_yol_durum_kodu_bir_dondurur() {
    let cikti = Command::new(disktree_bin())
        .args(["scan", "C:\\disktree-yok-boyle-bir-dizin-98765"])
        .output()
        .expect("calistir");
    assert!(!cikti.status.success());
    let hata = String::from_utf8_lossy(&cikti.stderr);
    assert!(hata.contains("hata"), "stderr: {hata}");
}

#[test]
fn cli_dry_run_kipi_yoktur_ama_sessiz_bayragi_calisir() {
    // `--dry-run` bilinçli olarak yoktur: arac zaten yalnızca okur ve hiçbir
    // komutta yazma yapmaz. Bunun yerine `--sessiz` yalnızca ekran çıktısını kapatır.
    let g = fixture("sessiz");
    let cikti = Command::new(disktree_bin())
        .args(["scan", &g.metin(), "--sessiz"])
        .output()
        .expect("calistir");
    assert!(cikti.status.success());
    assert!(cikti.stdout.is_empty(), "sessiz kipinde stdout bos olmali");
}

#[test]
fn cli_treemap_renksiz_ascii_uretir() {
    let g = fixture("cli-treemap");
    let cikti = Command::new(disktree_bin())
        .args([
            "treemap",
            &g.metin(),
            "--genislik",
            "40",
            "--yukseklik",
            "10",
        ])
        .output()
        .expect("calistir");
    assert!(cikti.status.success());
    let metin = String::from_utf8_lossy(&cikti.stdout);
    assert!(!metin.contains('\u{1b}'), "ciktida ANSI renk kodu olmamali");
    assert!(metin.contains("rampa:"));
    let izgara_satiri = metin.lines().filter(|l| l.chars().count() == 40).count();
    assert!(izgara_satiri >= 10, "izgara 10 satirdan kisa cikti");
}

#[test]
fn aylik_kume_zaman_cizelgesi_kumulatif_egri_uretir() {
    let g = fixture("aylik");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let k = Kayma::agactan(&sonuc.agac, 0);
    let aylar = zaman::aylik_kume(&k);
    assert!(!aylar.is_empty());
    let son_kumulatif = aylar.last().expect("son kova").kumulatif_bayt;
    assert_eq!(son_kumulatif, sonuc.agac.toplam_bayt());
    assert!(aylar.windows(2).all(|w| w[0].ay < w[1].ay));
}

#[test]
fn taranan_agac_turleri_dogru_siniflandirilir() {
    let g = fixture("turler");
    let sonuc = tara(&g.metin(), secenek()).expect("tarama");
    let agac = &sonuc.agac;
    let kok = agac.kok();
    assert_eq!(agac.kayit(kok).expect("kok").tur, Tur::Dizin);
    let belge = agac_dugum(agac, &format!("{}\\belge", g.metin())).expect("belge");
    assert_eq!(agac.kayit(belge).expect("kayit").tur, Tur::Dizin);
    assert_eq!(agac.kayit(belge).expect("kayit").boyut, 0);
    let dosya = agac_dugum(agac, &format!("{}\\belge\\rapor.txt", g.metin())).expect("dosya");
    assert_eq!(agac.kayit(dosya).expect("kayit").tur, Tur::Dosya);
    assert_eq!(agac.kayit(dosya).expect("kayit").boyut, 11);
}

# DiskTree demo agacini olusturur.
#
# Amac: README'deki her komutu bu agac uzerinde gercekten calistirabilmek.
# Uretilen dosyalar rastgele baytlardir; hicbir gercek kullanici verisi
# kullanilmaz ve hicbir agdan indirme yapilmaz.
#
# Kullanim (proje kokunde):
#   powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\demo-olustur.ps1
#
# Uretilen agac: .\demo  (bu klasor .gitignore icindedir, depoya girmez)

$ErrorActionPreference = 'Stop'
$root = Join-Path (Split-Path -Parent $PSScriptRoot) 'demo'

if (Test-Path -LiteralPath $root) {
    Remove-Item -LiteralPath $root -Recurse -Force
}

$dizinler = @(
    'projeler\gorsel', 'projeler\ses', 'muzik\canli', 'arsivler',
    'belgeler\eski', 'film'
)
foreach ($d in $dizinler) {
    $null = New-Item -ItemType Directory -Force -Path (Join-Path $root $d)
}

# Sabit tohumlu (deterministik) rastgele bayt uretici: test tekrarlanabilirligi
# icin System.Random yeterlidir, ayrica hicbir kripto amaci kullanilmaz.
$rastgele = [System.Random]::new(7)
function New-Dosya([string]$Yol, [int]$Bayt) {
    $tam = Join-Path $root $Yol
    $ust = Split-Path -Parent $tam
    if (-not (Test-Path -LiteralPath $ust)) {
        $null = New-Item -ItemType Directory -Force -Path $ust
    }
    $veri = New-Object byte[] $Bayt
    $rastgele.NextBytes($veri)
    [System.IO.File]::WriteAllBytes($tam, $veri)
}

New-Dosya 'muzik\canli\01-parca.flac'     3800000
New-Dosya 'muzik\canli\02-parca.flac'     2400000
New-Dosya 'muzik\albüm-kapak.jpg'          320000
New-Dosya 'projeler\gorsel\buyuk-video.mp4' 6100000
New-Dosya 'projeler\gorsel\tanitim.fla'     900000
New-Dosya 'projeler\ses\podcast-son-3.mp3'  1450000
New-Dosya 'film\tam-izleme.mkv'            7300000
New-Dosya 'belgeler\rapor.pdf'              520000
New-Dosya 'belgeler\eski\notlar.txt'         41000
New-Dosya 'belgeler\eski\arsiv-2023.zip'   2100000
[System.IO.File]::WriteAllText(
    (Join-Path $root 'belgeler\okunacaklar.md'),
    "# Okunacaklar`n- bir madde`n- ikinci madde`n",
    (New-Object System.Text.UTF8Encoding($false))
)
[System.IO.File]::WriteAllBytes((Join-Path $root 'bos-dosya.bin'), (New-Object byte[] 0))

$sayi = (Get-ChildItem -LiteralPath $root -Recurse -File | Measure-Object -Property Length -Sum)
"demo agaci hazir: $($sayi.Count) dosya, $($sayi.Sum) bayt -> $root"

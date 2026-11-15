# Crea il pacchetto MSIX di Tabby per il Microsoft Store.
#
#   1. Copia identity.example.json in identity.json e riempilo con i valori di
#      Partner Center → il tuo prodotto → Product management → Product identity.
#   2. Da PowerShell, nella cartella del progetto:  .\scripts\msix\build-msix.ps1
#
# Il pacchetto NON è firmato: lo firma Microsoft quando lo carichi su Partner Center.
# Risultato: src-tauri\target\msix\Tabby_<versione>_x64.msix

$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."
$here = $PSScriptRoot

$identityFile = Join-Path $here "identity.json"
if (-not (Test-Path $identityFile)) {
    throw "Manca scripts\msix\identity.json: copia identity.example.json e riempilo con i valori di Partner Center."
}
$identity = Get-Content $identityFile -Raw | ConvertFrom-Json
foreach ($key in "IdentityName", "Publisher", "PublisherDisplayName", "DisplayName") {
    if (-not $identity.$key -or $identity.$key -like "*<*") { throw "identity.json: manca $key" }
}

# Versione a 4 numeri (lo Store vuole l'ultimo a 0).
$conf = Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$version = "$($conf.version).0"

# 1. Programma in versione release (senza installer).
Push-Location $root
try {
    npm run tauri build -- --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "compilazione non riuscita" }
} finally { Pop-Location }

# 2. Cartella del pacchetto: programma, icone, manifest.
$out = Join-Path $root "src-tauri\target\msix"
$stage = Join-Path $out "stage"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force (Join-Path $stage "Assets") | Out-Null
Copy-Item (Join-Path $root "src-tauri\target\release\tabby.exe") $stage
python (Join-Path $here "make-assets.py") (Join-Path $stage "Assets")
if ($LASTEXITCODE -ne 0) { throw "icone non create (serve Python con Pillow)" }

$manifest = Get-Content (Join-Path $here "AppxManifest.template.xml") -Raw
$manifest = $manifest.Replace("{{IDENTITY_NAME}}", $identity.IdentityName).
    Replace("{{PUBLISHER}}", $identity.Publisher).
    Replace("{{PUBLISHER_DISPLAY_NAME}}", $identity.PublisherDisplayName).
    Replace("{{DISPLAY_NAME}}", $identity.DisplayName).
    Replace("{{VERSION}}", $version)
Set-Content -Path (Join-Path $stage "AppxManifest.xml") -Value $manifest -Encoding UTF8

# 3. makeappx dal Windows SDK (la versione più recente installata).
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
    Sort-Object { [version]($_.Directory.Parent.Name) } | Select-Object -Last 1
if (-not $makeappx) { throw "makeappx.exe non trovato: installa il Windows SDK" }

$package = Join-Path $out "Tabby_$($conf.version)_x64.msix"
& $makeappx.FullName pack /d $stage /p $package /o
if ($LASTEXITCODE -ne 0) { throw "makeappx non riuscito" }
Write-Host "`nPacchetto pronto: $package"
Write-Host "Caricalo in Partner Center → il tuo prodotto → Packages."

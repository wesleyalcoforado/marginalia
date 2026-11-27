<#
Monta dist\Marginalia-<versao>-win-x64.zip a partir do build de release (Windows x64).

Uso:  powershell -ExecutionPolicy Bypass -File scripts\empacotar_win.ps1

O zip e portatil: marginalia.exe com pdfium.dll ao lado (o app procura a biblioteca na pasta do executavel,
ver docs\pdfium.md) e a licenca do PDFium. Baixa a PDFium se faltar.
#>
$ErrorActionPreference = "Stop"
$raiz = Split-Path -Parent $PSScriptRoot
Push-Location $raiz
try {
    if (-not (Test-Path "vendor\pdfium\lib\pdfium.dll")) {
        python scripts\baixar_pdfium.py
        if ($LASTEXITCODE -ne 0) { throw "falha ao baixar a PDFium" }
    }
    cargo build --release -p ui
    if ($LASTEXITCODE -ne 0) { throw "falha no cargo build" }

    $versao = (Select-String -Path "crates\ui\Cargo.toml" -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches.Groups[1].Value
    $nome = "Marginalia-$versao-win-x64"
    $pasta = Join-Path "dist" $nome
    if (Test-Path $pasta) { Remove-Item -Recurse -Force $pasta }
    New-Item -ItemType Directory -Path $pasta | Out-Null

    Copy-Item "target\release\marginalia.exe" $pasta
    Copy-Item "vendor\pdfium\lib\pdfium.dll" $pasta
    Copy-Item "vendor\pdfium\LICENSE" (Join-Path $pasta "LICENSE-pdfium.txt")
# licenca, atribuicao do autor e licenca dos icones (a Apache-2.0 exige que acompanhem a redistribuicao)
Copy-Item "LICENSE", "NOTICE", "crates\ui\assets\icons\LICENSE-unicons.txt" $pasta

    $zip = "dist\$nome.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path $pasta -DestinationPath $zip
    Write-Host "Pronto: $zip"
} finally {
    Pop-Location
}

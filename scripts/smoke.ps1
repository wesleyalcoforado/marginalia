<#
Roda o smoke do Marginalia no Windows sem deixar variaveis de ambiente para tras.

Uso:  powershell -ExecutionPolicy Bypass -File scripts\smoke.ps1 <pasta-com-3-a-9-imagens>
      (o Bypass vale so para esta execucao: o Windows bloqueia scripts .ps1 por padrao; ou, uma vez,
       Set-ExecutionPolicy -Scope CurrentUser RemoteSigned)

Define MARGINALIA_SMOKE e MARGINALIA_APPDATA (pasta temporaria) so durante a execucao e as remove no fim (mesmo
com erro): no PowerShell `$env:X = ...` vale ate fechar o terminal, e o app passaria a rodar o smoke sempre.
O passo "Ir para" do smoke espera uma fonte de 1 a 9 imagens.
#>
param(
    [Parameter(Mandatory = $true)][string]$Fonte
)
$raiz = Split-Path -Parent $PSScriptRoot
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("marginalia-smoke-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $env:MARGINALIA_SMOKE = "1"
    $env:MARGINALIA_APPDATA = Join-Path $tmp "appdata"
    Push-Location $raiz
    cargo run -p ui -- $Fonte --vault (Join-Path $tmp "vault")
    $codigo = $LASTEXITCODE
} finally {
    Pop-Location -ErrorAction SilentlyContinue
    Remove-Item Env:MARGINALIA_SMOKE, Env:MARGINALIA_APPDATA -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
exit $codigo

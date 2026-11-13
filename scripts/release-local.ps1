# Plano B (Q28): build local + publicação manual com `gh`.
# Uso:  .\scripts\release-local.ps1 -Version 0.1.1 [-Publish]
# Requer: chave em $env:USERPROFILE\.tauri\ai-memory-manager.key e a senha digitada no prompt.
param(
  [Parameter(Mandatory)] [string] $Version,
  [switch] $Publish
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$keyPath = Join-Path $env:USERPROFILE ".tauri\ai-memory-manager.key"
if (-not (Test-Path $keyPath)) { throw "Chave do updater não encontrada em $keyPath (gere com: npx tauri signer generate -w `"$keyPath`")" }
$sec = Read-Host "Senha da chave do updater" -AsSecureString
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec))

# versão sincronizada em package.json, Cargo.toml e tauri.conf.json
npm version $Version --no-git-tag-version | Out-Null
(Get-Content src-tauri\Cargo.toml -Raw) -replace '(?m)^version = "[^"]+"', "version = `"$Version`"" | Set-Content -NoNewline src-tauri\Cargo.toml
$conf = Get-Content src-tauri\tauri.conf.json -Raw | ConvertFrom-Json
$conf.version = $Version
$conf | ConvertTo-Json -Depth 20 | Set-Content -Encoding utf8 src-tauri\tauri.conf.json

cargo test --lib --manifest-path src-tauri\Cargo.toml
npm test
npx tauri build

$nsis = Join-Path $root "src-tauri\target\release\bundle\nsis"
$setup = Get-ChildItem $nsis -Filter "*_${Version}_x64-setup.exe" | Select-Object -First 1
$sig = "$($setup.FullName).sig"
$tag = "v$Version"
$url = "https://github.com/lucasmonteir0/ai-memory-manager/releases/download/$tag/$($setup.Name)"
$latest = @{
  version  = $Version
  notes    = "AI Memory Manager $tag"
  pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  platforms = @{ "windows-x86_64" = @{ signature = (Get-Content $sig -Raw).Trim(); url = $url } }
} | ConvertTo-Json -Depth 5
$latestPath = Join-Path $nsis "latest.json"
Set-Content -Encoding utf8 $latestPath $latest
Write-Host "Gerado: $($setup.FullName)`n        $sig`n        $latestPath"

if ($Publish) {
  gh release create $tag $setup.FullName $sig $latestPath --repo lucasmonteir0/ai-memory-manager --title "AI Memory Manager $tag" --notes "Release $tag"
}

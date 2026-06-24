param(
  [string]$TargetTriple = "x86_64-pc-windows-msvc",
  [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$BackendTarget = Join-Path $RepoRoot "backend\target\$Profile"
$RuntimeTarget = Join-Path $RepoRoot "chawork-runtime\codex-rs\target\$Profile"
$BinariesDir = Join-Path $RepoRoot "backend\binaries"

New-Item -ItemType Directory -Force $BinariesDir | Out-Null

$Artifacts = @(
  @{
    Source = Join-Path $RuntimeTarget "chawork-runtime.exe"
    Target = Join-Path $BinariesDir "chawork-runtime-$TargetTriple.exe"
  },
  @{
    Source = Join-Path $RuntimeTarget "codex.exe"
    Target = Join-Path $BinariesDir "codex-$TargetTriple.exe"
  },
  @{
    Source = Join-Path $BackendTarget "chawork-mcp-server.exe"
    Target = Join-Path $BinariesDir "chawork-mcp-server-$TargetTriple.exe"
  }
)

foreach ($Artifact in $Artifacts) {
  if (!(Test-Path $Artifact.Source)) {
    throw "Missing sidecar source: $($Artifact.Source)"
  }
  Copy-Item -Force $Artifact.Source $Artifact.Target
  $Copied = Get-Item $Artifact.Target
  if ($Copied.Length -le 0) {
    throw "Copied sidecar is empty: $($Artifact.Target)"
  }
  Write-Host "Prepared $($Copied.FullName) ($($Copied.Length) bytes)"
}

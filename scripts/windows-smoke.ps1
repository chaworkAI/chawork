param(
  [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

function Join-ProcessArguments {
  param([string[]]$Arguments)

  return (($Arguments | ForEach-Object {
    if ($_ -match '[\s"]') {
      '"' + ($_ -replace '"', '\"') + '"'
    } else {
      $_
    }
  }) -join " ")
}

function Invoke-JsonLineProcess {
  param(
    [string]$FilePath,
    [string[]]$Arguments,
    [string[]]$InputLines,
    [int]$TimeoutMs = 30000
  )

  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = $FilePath
  $psi.Arguments = Join-ProcessArguments $Arguments
  $psi.RedirectStandardInput = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true

  $process = [System.Diagnostics.Process]::new()
  $process.StartInfo = $psi
  [void]$process.Start()

  foreach ($Line in $InputLines) {
    $process.StandardInput.WriteLine($Line)
  }
  $process.StandardInput.Close()

  if (!$process.WaitForExit($TimeoutMs)) {
    $process.Kill()
    throw "Process timed out: $FilePath"
  }

  $stdout = $process.StandardOutput.ReadToEnd()
  $stderr = $process.StandardError.ReadToEnd()
  if ($process.ExitCode -ne 0) {
    throw "Process failed: $FilePath`nExit: $($process.ExitCode)`nStdout: $stdout`nStderr: $stderr"
  }
  return $stdout
}

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$BackendTarget = Join-Path $RepoRoot "backend\target\$Profile"
$RuntimeTarget = Join-Path $RepoRoot "chawork-runtime\codex-rs\target\$Profile"

$RuntimeExe = Join-Path $RuntimeTarget "chawork-runtime.exe"
$CodexExe = Join-Path $RuntimeTarget "codex.exe"
$McpExe = Join-Path $BackendTarget "chawork-mcp-server.exe"

foreach ($Path in @($RuntimeExe, $CodexExe, $McpExe)) {
  if (!(Test-Path $Path)) {
    throw "Missing smoke-test executable: $Path"
  }
}

$Workspace = Join-Path ([System.IO.Path]::GetTempPath()) "ChaWork Smoke Workspace With Spaces"
New-Item -ItemType Directory -Force $Workspace | Out-Null

$RuntimeInit = @{
  id = 1
  method = "runtime/initialize"
  params = @{
    contractVersion = 1
    client = @{ name = "windows-smoke"; version = "0.1.0" }
    workspacePath = $Workspace
    requiredCapabilities = @()
  }
} | ConvertTo-Json -Compress -Depth 8

$RuntimeOut = Invoke-JsonLineProcess `
  -FilePath $RuntimeExe `
  -Arguments @("--protocol=jsonrpc") `
  -InputLines @($RuntimeInit)

if ($RuntimeOut -notmatch '"contractVersion"\s*:\s*1') {
  throw "runtime/initialize did not return contractVersion=1. Output: $RuntimeOut"
}

$McpInit = @{
  jsonrpc = "2.0"
  id = 1
  method = "initialize"
  params = @{
    protocolVersion = "2024-11-05"
    capabilities = @{}
    clientInfo = @{ name = "windows-smoke"; version = "0.1.0" }
  }
} | ConvertTo-Json -Compress -Depth 8

$McpTools = @{
  jsonrpc = "2.0"
  id = 2
  method = "tools/list"
  params = @{}
} | ConvertTo-Json -Compress -Depth 8

$McpOut = Invoke-JsonLineProcess `
  -FilePath $McpExe `
  -Arguments @("--workspace", $Workspace) `
  -InputLines @($McpInit, $McpTools)

if ($McpOut -notmatch '"tools"') {
  throw "MCP tools/list did not return tools. Output: $McpOut"
}

Write-Host "Windows runtime and MCP smoke checks passed."

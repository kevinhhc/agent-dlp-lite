param(
  [Parameter(Mandatory=$true)][string]$BinaryPath,
  [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$Sha256,
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'AgentDlpLite\bin'),
  [switch]$Force
)
$ErrorActionPreference = 'Stop'
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $BinaryPath).Hash -ne $Sha256) {
  throw 'Checksum mismatch; nothing installed'
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$target = Join-Path $InstallDir 'agent-dlp.exe'
if (Test-Path -LiteralPath $target) {
  $item = Get-Item -LiteralPath $target
  if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing linked destination' }
  if (!$Force) { throw 'Already installed; use -Force to replace' }
  Copy-Item -LiteralPath $target -Destination ($target + '.previous.' + [guid]::NewGuid().ToString('N'))
}
$temp = Join-Path $InstallDir ('.agent-dlp.' + [guid]::NewGuid().ToString('N') + '.tmp')
try {
  Copy-Item -LiteralPath $BinaryPath -Destination $temp
  Move-Item -LiteralPath $temp -Destination $target -Force
} finally {
  if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp }
}
Write-Output "Installed: $target"
Write-Output 'Add this directory to your user PATH if needed. Run agent-dlp self-test.'

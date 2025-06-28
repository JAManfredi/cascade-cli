$ErrorActionPreference = 'Stop'
$toolsDir = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$packageName = 'cascade-cli'

# Detect architecture
$arch = if ([Environment]::Is64BitOperatingSystem) { 'x64' } else { 'x86' }
$packageArgs = @{
  packageName   = $packageName
  unzipLocation = $toolsDir
  url64bit      = 'https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.0/cc-windows-x64.exe.zip'
  url           = 'https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.0/cc-windows-x64.exe.zip'
  checksum64    = 'TODO_REPLACE_WITH_ACTUAL_CHECKSUM'
  checksumType64= 'sha256'
  checksum      = 'TODO_REPLACE_WITH_ACTUAL_CHECKSUM'
  checksumType  = 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

# Create shim for the executable
$exePath = Join-Path $toolsDir 'cc.exe'
if (Test-Path $exePath) {
    Install-ChocolateyPath $toolsDir 'Machine'
} else {
    Write-Error "cc.exe not found in $toolsDir"
} 
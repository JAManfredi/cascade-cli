# Cascade CLI Windows Installation Script
# PowerShell script for installing Cascade CLI on Windows systems

param(
    [Parameter(HelpMessage="Installation directory (default: $env:USERPROFILE\bin)")]
    [string]$InstallDir = "$env:USERPROFILE\bin",
    
    [Parameter(HelpMessage="Force reinstallation even if already installed")]
    [switch]$Force,
    
    [Parameter(HelpMessage="Install shell completions")]
    [switch]$Completions,
    
    [Parameter(HelpMessage="Show verbose output")]
    [switch]$Verbose
)

# Set error handling
$ErrorActionPreference = "Stop"

# Colors for output
function Write-Success { param($Message) Write-Host "✅ $Message" -ForegroundColor Green }
function Write-Error { param($Message) Write-Host "❌ $Message" -ForegroundColor Red }
function Write-Warning { param($Message) Write-Host "⚠️ $Message" -ForegroundColor Yellow }
function Write-Info { param($Message) Write-Host "ℹ️ $Message" -ForegroundColor Blue }
function Write-Step { param($Message) Write-Host "📦 $Message" -ForegroundColor Blue }

Write-Host @"
🌊 Cascade CLI Installer for Windows
=====================================
"@ -ForegroundColor Cyan

# Check if running as administrator (optional but recommended)
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Warning "Not running as administrator. Some features may require elevated permissions."
}

# Detect architecture
$arch = if ([Environment]::Is64BitOperatingSystem) {
    if ([Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITEW6432") -eq "ARM64" -or 
        [Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE") -eq "ARM64") {
        "arm64"
    } else {
        "x64"
    }
} else {
    "x86"
}
Write-Info "Detected architecture: $arch"

# Check if already installed
$existingPath = Get-Command "ca" -ErrorAction SilentlyContinue
if ($existingPath -and -not $Force) {
    Write-Warning "Cascade CLI is already installed at: $($existingPath.Source)"
    Write-Info "Use -Force parameter to reinstall"
    return
}

try {
    # Create installation directory
    Write-Step "Creating installation directory..."
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    
    # Download latest release
    Write-Step "Downloading latest release..."
    $releaseUrl = "https://api.github.com/repos/JAManfredi/cascade-cli/releases/latest"
    
    try {
        $release = Invoke-RestMethod -Uri $releaseUrl -ErrorAction Stop
        $downloadUrl = $release.assets | Where-Object { $_.name -like "*windows-$arch*" } | Select-Object -ExpandProperty browser_download_url -First 1
        
        if (-not $downloadUrl) {
            throw "No Windows binary found for architecture $arch"
        }
        
        Write-Info "Downloading from: $downloadUrl"
    }
    catch {
        Write-Error "Failed to fetch release information: $($_.Exception.Message)"
        Write-Info "Falling back to manual download..."
        $downloadUrl = "https://github.com/JAManfredi/cascade-cli/releases/latest/download/ca-windows-$arch.exe.zip"
    }
    
    $zipPath = Join-Path $InstallDir "ca-windows-$arch.exe.zip"
    $exePath = Join-Path $InstallDir "ca.exe"
    
    # Download with progress
    $webClient = New-Object System.Net.WebClient
    $webClient.DownloadProgressChanged += {
        param($sender, $e)
        Write-Progress -Activity "Downloading Cascade CLI" -Status "Progress" -PercentComplete $e.ProgressPercentage
    }
    
    try {
        $webClient.DownloadFile($downloadUrl, $zipPath)
        Write-Progress -Activity "Downloading Cascade CLI" -Completed
        Write-Success "Download completed"
    }
    catch {
        Write-Error "Download failed: $($_.Exception.Message)"
        throw
    }
    finally {
        $webClient.Dispose()
    }
    
    # Extract binary
    Write-Step "Extracting binary..."
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
        
        $entry = $zip.Entries | Where-Object { $_.Name -eq "ca.exe" } | Select-Object -First 1
        if ($entry) {
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $exePath, $true)
            Write-Success "Binary extracted to: $exePath"
        }
        else {
            throw "ca.exe not found in archive"
        }
    }
    catch {
        Write-Error "Extraction failed: $($_.Exception.Message)"
        throw
    }
    finally {
        if ($zip) { $zip.Dispose() }
        Remove-Item $zipPath -ErrorAction SilentlyContinue
    }
    
    # Verify binary
    Write-Step "Verifying installation..."
    try {
        $version = & $exePath version 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Success "Installation verified: $version"
        }
        else {
            throw "Binary verification failed"
        }
    }
    catch {
        Write-Error "Binary verification failed: $($_.Exception.Message)"
        throw
    }
    
    # Add to PATH
    Write-Step "Updating PATH..."
    $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($userPath -notlike "*$InstallDir*") {
        $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
        [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
        Write-Success "Added to user PATH"
        Write-Warning "Restart your shell for PATH changes to take effect"
    }
    else {
        Write-Info "Installation directory already in PATH"
    }
    
    # Install completions if requested
    if ($Completions) {
        Write-Step "Installing shell completions..."
        try {
            & $exePath completions install
            Write-Success "Shell completions installed"
        }
        catch {
            Write-Warning "Failed to install completions: $($_.Exception.Message)"
        }
    }
    
    Write-Host @"

🎉 Installation Complete!
========================

Cascade CLI has been installed to: $InstallDir

📋 Next steps:
1. Restart your shell (PowerShell/Command Prompt)
2. Navigate to your Git repository
3. Run: ca setup

💡 Quick start:
   ca init --bitbucket-url https://your-bitbucket-server.com
   ca stacks create my-feature --base main
   git commit -m "Add new feature"
   ca push && ca submit

📚 Documentation: https://github.com/JAManfredi/cascade-cli
"@ -ForegroundColor Green

}
catch {
    Write-Error "Installation failed: $($_.Exception.Message)"
    
    # Cleanup on failure
    if (Test-Path $InstallDir) {
        try {
            Remove-Item $InstallDir -Recurse -Force
            Write-Info "Cleaned up failed installation"
        }
        catch {
            Write-Warning "Failed to cleanup: $($_.Exception.Message)"
        }
    }
    
    exit 1
}
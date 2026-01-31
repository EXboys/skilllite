# Skillbox Installation Script for Windows
# Usage: iwr -useb https://raw.githubusercontent.com/EXboys/skilllite/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "EXboys/skilllite"
$BinaryName = "skillbox"
$InstallDir = if ($env:SKILLBOX_INSTALL_DIR) { $env:SKILLBOX_INSTALL_DIR } else { "$env:LOCALAPPDATA\Programs\skillbox" }

function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Get-LatestRelease {
    Write-ColorOutput Yellow "Fetching latest release..."
    
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
        $version = $response.tag_name
        Write-ColorOutput Green "Latest version: $version"
        return $version
    }
    catch {
        Write-ColorOutput Red "Failed to fetch latest release"
        exit 1
    }
}

function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -eq "AMD64" -or $arch -eq "x86_64") {
        return "x86_64"
    }
    elseif ($arch -eq "ARM64") {
        return "arm64"
    }
    else {
        Write-ColorOutput Red "Unsupported architecture: $arch"
        exit 1
    }
}

function Download-Binary($version, $arch) {
    $binaryFile = "$BinaryName-windows-$arch.exe"
    $downloadUrl = "https://github.com/$Repo/releases/download/$version/$binaryFile"
    $tempFile = "$env:TEMP\$binaryFile"
    
    Write-ColorOutput Yellow "Downloading from: $downloadUrl"
    
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tempFile -UseBasicParsing
        Write-ColorOutput Green "Download completed"
        return $tempFile
    }
    catch {
        Write-ColorOutput Red "Download failed: $_"
        exit 1
    }
}

function Install-Binary($tempFile) {
    # Create install directory if it doesn't exist
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    
    $installPath = Join-Path $InstallDir "$BinaryName.exe"
    
    # Remove old version if exists
    if (Test-Path $installPath) {
        Remove-Item $installPath -Force
    }
    
    # Move binary to install directory
    Move-Item $tempFile $installPath -Force
    
    Write-ColorOutput Green "Installed to: $installPath"
    
    # Add to PATH if not already there
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$InstallDir*") {
        Write-ColorOutput Yellow "Adding $InstallDir to PATH..."
        [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
        Write-ColorOutput Green "Added to PATH. Please restart your terminal."
    }
}

# Main installation process
function Main {
    Write-ColorOutput Green "=== Skillbox Installation ==="
    
    $arch = Get-Architecture
    Write-ColorOutput Green "Detected architecture: $arch"
    
    $version = Get-LatestRelease
    $tempFile = Download-Binary $version $arch
    Install-Binary $tempFile
    
    Write-ColorOutput Green "=== Installation Complete ==="
    Write-ColorOutput Green "Run 'skillbox --help' to get started"
    Write-ColorOutput Yellow "Note: You may need to restart your terminal for PATH changes to take effect"
}

Main


# build-windows.ps1
# Media Manager - Automated Windows Build Script

$ErrorActionPreference = "Stop"
$StartTime = Get-Date

function Write-Host-Color($Message, $Color = "White") {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Message" -ForegroundColor $Color
}

Write-Host-Color "===============================================" "Cyan"
Write-Host-Color "   MEDIA MANAGER - WINDOWS BUILD SYSTEM        " "Cyan"
Write-Host-Color "===============================================" "Cyan"

try {
    # 1. Prerequisite Checks
    Write-Host-Color "Step 1: Checking Prerequisites..." "Yellow"
    
    if (!(Get-Command npm -ErrorAction SilentlyContinue)) { throw "Node.js/NPM not found. Please install Node.js." }
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) { throw "Rust/Cargo not found. Please install Rust." }
    
    if (!(Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
        Write-Host-Color "Tauri CLI not found. Installing..." "Magenta"
        cargo install tauri-cli --version "^2.0.0"
    }

    # 2. Cleanup
    Write-Host-Color "Step 2: Cleaning old build artifacts..." "Yellow"
    if (Test-Path "frontend/dist") { Remove-Item -Recurse -Force "frontend/dist" }
    
    # 3. Frontend Build
    Write-Host-Color "Step 3: Building React Frontend..." "Yellow"
    Set-Location frontend
    npm install
    npm run build
    Set-Location ..
    Write-Host-Color "Frontend build successful." "Green"

    # 4. Desktop App Build
    Write-Host-Color "Step 4: Compiling Rust Desktop App & Bundling..." "Yellow"
    Set-Location apps/desktop
    
    # We use --verbose to get detailed logs
    cargo tauri build
    
    Set-Location ../..

    # 5. Summary
    $Duration = (Get-Date) - $StartTime
    Write-Host-Color "===============================================" "Cyan"
    Write-Host-Color "BUILD COMPLETED SUCCESSFULLY!" "Green"
    Write-Host-Color "Total Time: $($Duration.Minutes)m $($Duration.Seconds)s" "Cyan"
    
    $BinaryPath = "target/release/desktop.exe"
    $InstallerPath = Get-ChildItem "target/release/bundle/msi/*.msi" | Select-Object -ExpandProperty FullName -First 1
    
    Write-Host ""
    Write-Host "Outputs:" -ForegroundColor "Yellow"
    Write-Host "Standalone EXE: $BinaryPath" -ForegroundColor "White"
    if ($InstallerPath) {
        Write-Host "MSI Installer:  $InstallerPath" -ForegroundColor "White"
    }
    Write-Host-Color "===============================================" "Cyan"

} catch {
    Write-Host ""
    Write-Host "BUILD FAILED!" -ForegroundColor "Red"
    Write-Host "Error: $($_.Exception.Message)" -ForegroundColor "Red"
    Set-Location $PSScriptRoot
    exit 1
}

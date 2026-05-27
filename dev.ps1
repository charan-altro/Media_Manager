param (
    [ValidateSet("web", "desktop", "test", "docker")]
    [string]$Mode = "web"
)

$RootDir = $PSScriptRoot

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "  Media Orchestrator Automation Script" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

switch ($Mode) {
    "test" {
        Write-Host "`n[+] Running all Rust workspace tests..." -ForegroundColor Green
        Set-Location $RootDir
        cargo test --workspace
        break
    }
    "desktop" {
        Write-Host "`n[+] Starting Tauri Desktop Application..." -ForegroundColor Green
        Set-Location "$RootDir\frontend"
        Write-Host "Installing frontend dependencies..." -ForegroundColor DarkGray
        npm install
        
        Set-Location "$RootDir\apps\desktop"
        Write-Host "Launching Tauri Dev Server..." -ForegroundColor DarkGray
        cargo tauri dev
        break
    }
    "docker" {
        Write-Host "`n[+] Building and starting Docker container..." -ForegroundColor Green
        Set-Location $RootDir
        docker compose up --build
        break
    }
    "web" {
        Write-Host "`n[+] Starting Web Development Environment..." -ForegroundColor Green
        Set-Location $RootDir

        Write-Host "1. Starting Axum Backend Server..." -ForegroundColor Yellow
        $env:RUST_LOG = "info,media_core=debug,server=debug"
        # Start backend in the same console window
        $BackendProcess = Start-Process -FilePath "cargo" -ArgumentList "run", "-p", "server" -NoNewWindow -PassThru

        Write-Host "2. Starting React Frontend..." -ForegroundColor Yellow
        Set-Location "$RootDir\frontend"
        npm install
        
        try {
            # This will block the terminal and run Vite
            npm run dev
        } finally {
            # Cleanup when Vite is stopped (Ctrl+C)
            Write-Host "Stopping Backend Server..." -ForegroundColor Yellow
            if ($BackendProcess -and !$BackendProcess.HasExited) {
                Stop-Process -Id $BackendProcess.Id -Force
            }
            Stop-Process -Name server -Force -ErrorAction SilentlyContinue
        }
        break
    }
}

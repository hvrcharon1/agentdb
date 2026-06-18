# AgentDB installer for Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/hvrcharon1/agentdb/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo = "hvrcharon1/agentdb"
$InstallDir = if ($env:AGENTDB_INSTALL_DIR) { $env:AGENTDB_INSTALL_DIR } else { "$env:LOCALAPPDATA\agentdb" }

function Info($msg) { Write-Host "=> $msg" -ForegroundColor Cyan }
function Error($msg) { Write-Host "error: $msg" -ForegroundColor Red; exit 1 }

# Get latest version
$Version = if ($env:AGENTDB_VERSION) { $env:AGENTDB_VERSION } else {
    $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
    $release.tag_name -replace '^v', ''
}

if (-not $Version) { Error "Could not determine latest version" }

$Url = "https://github.com/$Repo/releases/download/v$Version/agentdb-x86_64-pc-windows-msvc.zip"
$ChecksumUrl = "https://github.com/$Repo/releases/download/v$Version/checksums-sha256.txt"

Info "Downloading agentdb v$Version for Windows x86_64..."

$TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "agentdb-install-$(Get-Random)")
try {
    $ZipPath = Join-Path $TmpDir "agentdb.zip"
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing

    # Verify checksum
    $Checksums = (Invoke-WebRequest -Uri $ChecksumUrl -UseBasicParsing).Content
    $Expected = ($Checksums -split "`n" | Where-Object { $_ -match 'x86_64-pc-windows-msvc' }) -replace '\s+.*', ''
    $Actual = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLower()

    if ($Expected -ne $Actual) {
        Error "Checksum mismatch!`n  Expected: $Expected`n  Actual:   $Actual"
    }
    Info "Checksum verified."

    # Extract
    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

    # Install
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Copy-Item (Join-Path $TmpDir "agentdb.exe") (Join-Path $InstallDir "agentdb.exe") -Force

    # Add to PATH if not already there
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
        $env:Path = "$env:Path;$InstallDir"
        Info "Added $InstallDir to PATH."
    }

    Info "Installed agentdb v$Version to $InstallDir\agentdb.exe"
    Info "Run 'agentdb --version' to verify."
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}

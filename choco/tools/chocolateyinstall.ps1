$ErrorActionPreference = 'Stop'

$packageName = 'agentdb'
$version     = '0.6.0'
$url64       = "https://github.com/hvrcharon1/agentdb/releases/download/v${version}/agentdb-x86_64-pc-windows-msvc.zip"

$packageArgs = @{
  packageName   = $packageName
  unzipLocation = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
  url64bit      = $url64
  checksum64    = 'PLACEHOLDER'
  checksumType64= 'sha256'
}

Install-ChocolateyZipPackage @packageArgs

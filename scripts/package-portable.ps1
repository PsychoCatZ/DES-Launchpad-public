param([string]$OutputFolder = 'release/DES Launchpad')
$ErrorActionPreference='Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
node scripts/check-public-defaults.cjs
if ($LASTEXITCODE -ne 0) { throw 'Public defaults check failed' }
$taskRelease = Join-Path $PWD 'src-tauri/target/release'
$taskPortable = Join-Path $PWD $OutputFolder
$taskVersion = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
$taskInstaller = "DES Launchpad Public_${taskVersion}_x64-setup.exe"
New-Item -ItemType Directory -Force -Path $taskPortable | Out-Null
Copy-Item -LiteralPath (Join-Path $taskRelease 'des-launchpad.exe') -Destination $taskPortable -Force
Copy-Item -LiteralPath (Join-Path $taskRelease 'defaults') -Destination $taskPortable -Recurse -Force
Copy-Item -LiteralPath (Join-Path $taskRelease "bundle/nsis/$taskInstaller") -Destination 'release' -Force
Copy-Item -LiteralPath 'LICENSE' -Destination $taskPortable -Force
Copy-Item -LiteralPath 'README.md' -Destination $taskPortable -Force
Copy-Item -LiteralPath 'VALIDATION.md' -Destination $taskPortable -Force
Get-FileHash -LiteralPath (Join-Path $taskPortable 'des-launchpad.exe'),(Join-Path $PWD "release/$taskInstaller") -Algorithm SHA256 | Format-Table -AutoSize

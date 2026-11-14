# Live tuning: puts data/mobs.json where `/sc reload` reads it (the joint server's
# config/soulscraft/mobs.json) and reloads it on the running server. No rebuild: numbers and
# timings change for the next attack. Mob ids must stay the same (a new mob needs a restart).
#   .\prototypes\tune.ps1           copy and reload
#   .\prototypes\tune.ps1 -Clear    remove the file: the server goes back to the built-in numbers
param([switch]$Clear)
$ErrorActionPreference = 'Stop'
$Repo = Split-Path $PSScriptRoot
$Server = "$env:LOCALAPPDATA\Horizon\joint-dev\server"
$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$Target = Join-Path $Server 'config\soulscraft\mobs.json'
if ($Clear) {
    if (Test-Path $Target) { Remove-Item -LiteralPath $Target }
    Write-Host "Removed $Target (takes effect at the next server start)"
    return
}
New-Item -ItemType Directory -Force (Split-Path $Target) | Out-Null
Copy-Item -LiteralPath (Join-Path $Repo 'data\mobs.json') -Destination $Target
Write-Host "Copied to $Target"
& $ServerTool --dir $Server cmd 'sc reload'

# П5's own server (round 8: a separate world without natural spawning, so costs compare like
# with like). It runs the joint bench's current server engine and content (horizon-joint-dev:
# the same build, all its mods) on its own folder, port and world, and never touches the joint
# server or its world:
#   F:\horizon-servers\soulscraft-p5 (the Dev Drive, beside the joint server: no Defender scan of world and log writes), port 25571, 127.0.0.1 only, no web panel,
#   offline (bots sc_a, sc_b), flat world `p5` with a fixed seed, made fresh at every start.
# After each start: no natural spawning (spawn_mobs, spawn_monsters, patrols, phantoms, traders,
# wardens off), the clock and weather stopped at a clear midnight (vanilla zombies don't burn),
# difficulty normal, and the server never pauses when empty.
#   .\prototypes\p5-world.ps1 init       once: make the server folder
#   .\prototypes\p5-world.ps1 start      engine and content from the joint server, fresh world, rules
#   .\prototypes\p5-world.ps1 stop
#   .\prototypes\p5-world.ps1 status
# No native build here: build with `.\joint.ps1 server` first. Start it only while no native build
# runs and %LOCALAPPDATA%\Horizon\joint-dev\maintenance.txt is absent (the bench's rules).
param([ValidateSet('init', 'start', 'stop', 'status')][string]$Part = 'status')
$ErrorActionPreference = 'Stop'

$ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$Joint = "$env:LOCALAPPDATA\Horizon\joint-dev"
$JointServer = "$Joint\server"
$Server = "F:\horizon-servers\soulscraft-p5"
$Port = 25571
$World = 'p5'
$Seed = 'soulscraft-p5'
$StartTimeout = 180
# Natural spawning and everything else that adds mobs on its own.
$Rules = @('spawn_mobs false', 'spawn_monsters false', 'spawn_patrols false', 'spawn_phantoms false',
    'spawn_wandering_traders false', 'spawn_wardens false', 'advance_time false', 'advance_weather false')

function Invoke-Tool([string[]]$Arguments) {
    $out = & $ServerTool --dir $Server @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw "horizon-server $($Arguments -join ' '): $out" }
    $out
}

function Test-Up {
    & $ServerTool --dir $Server cmd 'list' 2>$null | Out-Null
    $LASTEXITCODE -eq 0
}

function Assert-Quiet {
    if (Test-Path "$Joint\maintenance.txt") { throw "Bench maintenance: $(Get-Content "$Joint\maintenance.txt")" }
}

function Initialize-P5 {
    if (Test-Path "$Server\server.toml") { Write-Host "P5 server exists: $Server"; return }
    New-Item -ItemType Directory -Force $Server | Out-Null
    Invoke-Tool @('init', '--eula', '--world', $World, '--seed', $Seed, '--port', "$Port", '--no-whitelist', '--offline', '--yes') | Out-Null
    Invoke-Tool @('config', 'set', 'server.bind=127.0.0.1', 'panel.bind=', 'world.game_mode=creative',
        'world.difficulty=normal', 'server.online_mode=false', 'server.accept_older_clients=false',
        'properties.level-type=minecraft:flat', 'properties.pause-when-empty-seconds=0', 'backups.every=0') | Out-Null
    Write-Host "P5 server made: $Server (port $Port, world $World, seed $Seed)"
}

# The joint server's engine and installed content: the very build the bench made.
function Sync-FromJoint {
    $engine = (Select-String -Path "$JointServer\server.toml" -Pattern "^path = '(.+)'").Matches[0].Groups[1].Value
    if (-not (Test-Path $engine)) { throw "No joint engine at $engine" }
    Invoke-Tool @('config', 'set', "engine.path=$engine") | Out-Null
    $mods = "$Server\horizon-mods"
    if (Test-Path $mods) { Remove-Item -LiteralPath $mods -Recurse -Force }
    Copy-Item -LiteralPath "$JointServer\horizon-mods" -Destination $mods -Recurse
    Copy-Item -LiteralPath "$JointServer\horizon-mods.txt" -Destination "$Server\horizon-mods.txt" -Force
    $engine
}

function Start-P5 {
    Assert-Quiet
    if (-not (Test-Path "$Server\server.toml")) { throw 'No P5 server: .\prototypes\p5-world.ps1 init' }
    if (Test-Up) { throw 'The P5 server runs already: stop it first' }
    $engine = Sync-FromJoint
    # The same flat world from the same seed at every start: like with like.
    if (Test-Path "$Server\$World") { Remove-Item -LiteralPath "$Server\$World" -Recurse -Force }
    $logs = New-Item -ItemType Directory -Force "$Server\logs"
    # `start` leaves the supervisor running with the output it inherited: only `start` itself is
    # waited for (Start-Process -Wait would wait for the supervisor too), as joint.ps1 does.
    $starter = Start-Process $ServerTool -ArgumentList '--dir', "`"$Server`"", 'start' -NoNewWindow -PassThru `
        -RedirectStandardOutput "$logs\p5-start.out" -RedirectStandardError "$logs\p5-start.err"
    $starter.WaitForExit()
    $deadline = (Get-Date).AddSeconds($StartTimeout)
    while (-not (Test-Up)) {
        if ((Get-Date) -gt $deadline) { throw "The P5 server did not answer in $StartTimeout s; see $Server\logs" }
        Start-Sleep -Milliseconds 500
    }
    foreach ($r in $Rules) { Invoke-Tool @('cmd', "gamerule $r") | Out-Null }
    foreach ($c in 'time set midnight', 'weather clear', 'difficulty normal', 'kill @e[type=!player]') {
        Invoke-Tool @('cmd', $c) | Out-Null
    }
    Write-Host "P5 server up on 127.0.0.1:$Port, fresh world $World (seed $Seed), engine $engine"
}

switch ($Part) {
    'init' { Initialize-P5 }
    'start' { Start-P5 }
    'stop' { if (Test-Up) { Invoke-Tool @('stop') | Out-Null; Write-Host 'P5 server stopped' } else { Write-Host 'P5 server is not running' } }
    'status' { if (Test-Path "$Server\server.toml") { & $ServerTool --dir $Server status } else { Write-Host "No P5 server yet ($Server)" } }
}

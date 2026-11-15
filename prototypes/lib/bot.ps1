# Offline bot clients for the joint bench (horizon-joint-dev/README.md): SoulsCraft's bots are
# sc_a and sc_b, its area x −1000..0, z 0..1000. Dot-source it:
#   . .\prototypes\lib\bot.ps1
#   $a = Start-Bot sc_a
#   Invoke-Bot $a 'state'
#   Stop-Bot $a
# The server must run (`.\joint.ps1 serve -Owner "soulscraft agent"`) in offline mode
# (`.\joint.ps1 open`, and `close` after). Commands that change the world go through the server's
# console (Invoke-Console), so the bots need no operator rights.
$script:Ctl = "$env:LOCALAPPDATA\Horizon\current\horizon-ctl.exe"
$script:ServerTool = "$env:LOCALAPPDATA\Horizon\current\horizon-server.exe"
$script:JointServer = "$env:LOCALAPPDATA\Horizon\joint-dev\server"
$script:BenchPort = 25570
$script:Instance = "$env:APPDATA\horizon\instances\joint-dev"
# Bot homes, logs and full-size screenshots; set SOULSCRAFT_SCRATCH to keep them elsewhere.
$script:Scratch = if ($env:SOULSCRAFT_SCRATCH) { $env:SOULSCRAFT_SCRATCH } else { Join-Path $env:TEMP 'soulscraft-bench' }

function Get-JointClient {
    $instances = Get-Content -Raw "$env:APPDATA\horizon-launcher\instances.json" | ConvertFrom-Json
    $version = $instances.instances.'joint-dev'.version
    if (-not $version) { throw 'joint-dev plays no native build yet: .\joint.ps1 client' }
    $exe = "$env:APPDATA\horizon-launcher\versions\$version\horizon-client.exe"
    if (-not (Test-Path $exe)) { throw "No client at $exe" }
    $exe
}

# Starts an offline client named $Name on the joint server, offscreen (screenshots still work).
function Start-Bot([string]$Name, [switch]$Still) {
    $dir = Join-Path $script:Scratch "bots\$Name"
    New-Item -ItemType Directory -Force $dir | Out-Null
    $socket = Join-Path $dir 'ctl.sock'
    # Owner's rule: bots play without sound. Volume lives in the player dir's options.txt (a fresh
    # one starts at 1.0): the master volume is set to 0 there before every launch.
    $options = Join-Path $dir 'options.txt'
    $lines = if (Test-Path $options) { @(Get-Content $options | Where-Object { $_ -notmatch '^soundCategory_master:' }) } else { @() }
    [IO.File]::WriteAllLines($options, [string[]](@($lines) + 'soundCategory_master:0.0'), [Text.UTF8Encoding]::new($false))
    if ($Still) { $env:HORIZON_STILL = '1' } else { Remove-Item Env:HORIZON_STILL -ErrorAction SilentlyContinue }
    $reply = & $script:Ctl --socket $socket launch --game (Get-JointClient) --offscreen --home $script:Instance `
        --game-dir $script:Instance --player-dir $dir --name $Name --server "127.0.0.1:$script:BenchPort" 2>&1
    Remove-Item Env:HORIZON_STILL -ErrorAction SilentlyContinue
    if ($LASTEXITCODE -ne 0) { throw "launch of ${Name} failed: $reply" }
    $bot = [pscustomobject]@{ Name = $Name; Socket = $socket; Dir = $dir }
    Invoke-Bot $bot 'wait-ready 120' | Out-Null
    # The scripts read per-mob lines (adopt, phase, strike, boost, trophy particles) from the
    # server log: since 0.3.0 they are at `debug` unless `/sc log on`.
    Invoke-Console 'sc log on' | Out-Null
    $bot
}

function Invoke-Bot($Bot, [string]$Line, [int]$Timeout = 60) {
    $words = $Line -split ' '
    $out = & $script:Ctl --socket $Bot.Socket --timeout $Timeout @words 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "$($Bot.Name): $Line -> $out" }
    $out
}

function Stop-Bot($Bot) {
    Invoke-Bot $Bot 'release' | Out-Null
    & $script:Ctl --socket $Bot.Socket quit 2>&1 | Out-Null
}

# Points the bots and the console at П5's own server (prototypes/p5-world.ps1) instead of the
# joint one.
function Use-P5Server {
    $script:JointServer = "F:\horizon-servers\soulscraft-p5"
    $script:BenchPort = 25571
}

# A console command on the joint server (`/sc …` included: the console runs as operator 4).
function Invoke-Console([string]$Line) {
    $out = & $script:ServerTool --dir $script:JointServer cmd $Line 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "console: $Line -> $out" }
    $out
}

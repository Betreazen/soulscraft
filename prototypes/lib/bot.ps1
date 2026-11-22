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
$script:InstanceName = "joint-dev"
# Set by lib\tidewill.ps1 (Use-Tidewill): bots go to the live server, the console over ssh.
$script:ServerAddress = $null
$script:Remote = $false
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
    $server = if ($script:ServerAddress) { $script:ServerAddress } else { "127.0.0.1:$script:BenchPort" }
    if ($script:ModApi) {
        $reply = & $script:Ctl --socket $socket launch --game $script:ModApiClient --offscreen --resources $script:ModApiResources `
            --home $script:Instance --game-dir $script:Instance --player-dir $dir --name $Name --server $server 2>&1
    } else {
        $reply = & $script:Ctl --socket $socket launch --game (Get-JointClient) --offscreen --home $script:Instance `
            --game-dir $script:Instance --player-dir $dir --name $Name --server $server 2>&1
    }
    Remove-Item Env:HORIZON_STILL -ErrorAction SilentlyContinue
    if ($LASTEXITCODE -ne 0) { throw "launch of ${Name} failed: $reply" }
    if ($script:ModApi) {
        try {
            $j = ($reply -join '') | ConvertFrom-Json
            $botPid = if ($j.pid) { $j.pid } else { $j.launch.pid }
            [JointCpuCap]::Assign($script:CpuJob, (Get-Process -Id $botPid).Handle); Add-Descendants $script:CpuJob $botPid
        } catch { Write-Warning "CPU cap: $_" }
    }
    $bot = [pscustomobject]@{ Name = $Name; Socket = $socket; Dir = $dir }
    Invoke-Bot $bot 'wait-ready 120' | Out-Null
    # The scripts read per-mob lines (adopt, phase, strike, boost, trophy particles) from the
    # server log: since 0.3.0 they are at `debug` unless `/sc log on`.
    # On the live server the playtest turns them on once and off at the end.
    if (-not $script:Remote) { Invoke-Console 'sc log on' | Out-Null }
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
    if ($script:Remote) { return (Invoke-Remote @("cmd $Line"))[0] }
    if ($script:ModApi) {
        $q = $Line -replace "'", "'\''"
        $out = & $script:Hlx -Sh "$($script:BoxServerTool) --dir $($script:BoxServer) cmd '$q'" 2>&1 | Where-Object { $_ -notmatch '^hlx: ' }
        return $out
    }
    $out = & $script:ServerTool --dir $script:JointServer cmd $Line 2>&1
    if ($LASTEXITCODE -ne 0) { Write-Warning "console: $Line -> $out" }
    $out
}

# The owner's 87% CPU cap for the Mod API bench's bots (the job object of joint.ps1, also used by
# hl.ps1): loaded at the script's top level, so Add-Descendants is seen where Start-Bot runs.
if (Test-Path 'C:\Claude code projects\horizon-joint-dev\joint.ps1') {
    . ([scriptblock]::Create(((Get-Content 'C:\Claude code projects\horizon-joint-dev\joint.ps1')[273..326] -join "`n")))
}

# The Mod API bench (mod/README.md): the server is the Linux export in the horizon-linux-dev box
# (/hz/servers/soulscraft, 127.0.0.1:25591), the bots are offline clients of the isolated 8f83e5c
# install in horizon-sdk-dev playing the native build $Version, muted, offscreen and under the
# owner's 87% CPU cap. Call it after dot-sourcing this file and before Start-Bot.
function Use-ModApiBench([Parameter(Mandatory)][string]$Version, [Parameter(Mandatory)][string]$Export) {
    $sdk = 'C:\Claude code projects\horizon-sdk-dev'
    $env:HORIZON_LAUNCHER_HOME = "$sdk\launcher"; $env:HORIZON_HOME = "$sdk\game"; $env:HORIZON_LAUNCHER_NO_SELF_UPDATE = '1'
    $script:ModApi = $true
    $script:Hlx = 'C:\Claude code projects\horizon-linux-dev\hlx.ps1'
    $script:BoxServer = '/hz/servers/soulscraft'
    $script:BoxServerTool = "/hz/exports/$Export/horizon-server"
    $script:Ctl = "$sdk\launcher\versions\$Version\horizon-ctl.exe"
    $script:ModApiClient = "$sdk\launcher\versions\$Version\horizon-client.exe"
    $script:ModApiResources = "$sdk\launcher\versions\nightly-2026-11-21-8f83e5c\.local\horizon-client\26.3"
    $script:Instance = "$sdk\bench\sc-bots"
    $script:BenchPort = 25591
    New-Item -ItemType Directory -Force $script:Instance | Out-Null
    $script:CpuJob = [JointCpuCap]::Create(87)
}

# The server log's lines on the Mod API bench (read from the box each call).
function Get-ServerLog {
    if ($script:ModApi) {
        return @(& $script:Hlx -Sh "cat $($script:BoxServer)/logs/latest.log" 2>&1 | Where-Object { $_ -notmatch '^hlx: ' })
    }
    @(Get-Content "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log")
}

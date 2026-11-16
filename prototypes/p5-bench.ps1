# П5 (hypothesis Г8): what smart mobs round one player cost the server, like with like.
# Each run starts П5's own server on a fresh flat world from the same seed without natural
# spawning (prototypes/p5-world.ps1: the joint bench's engine and content, its own folder and port),
# connects the bot sc_a (survival under Resistance V: the mobs fight it for real, it takes no
# damage) in the test arena, and measures four cases in turn, each sampled for $Seconds after
# $Settle seconds:
#   revenants   $Count revenants at P 0.5 (/sc bench)
#   mixed       all seven mobs in turn, 7 of each (/sc bench 49 mixed)
#   zombies     $Count vanilla zombies (midnight, the clock stopped: none burn)
#   empty       the arena with the bot only
#   crowd       a live server's mix (12-playtest-tidewill.md): $Crowd adopted vanilla monsters
#               (zombies, skeletons, spiders in turn) round the bot and $Herd cows farther out,
#               which SoulsCraft never adopts but every search through the level's mobs passes
# Per case: the server's average tick (native_tick_stats), SoulsCraft's own time per server tick,
# whole and by part (cost.rs), and each mob type's own ticks (native_mob_cost_sampling, summed by
# type). After $Runs runs it prints the median and the spread (min..max) of each number, and
# writes everything to prototypes/logs/p5-<time>.txt.
#   .\prototypes\p5-world.ps1 init         once
#   .\prototypes\p5-bench.ps1 [-Runs 3] [-Seconds 30] [-Settle 10] [-Count 50] [-Cases crowd,empty]
#                             [-Engine PATH] [-Label before]
# -Engine runs another engine than the joint server's (p5-world.ps1), -Label names the log file.
# Only on a quiet PC: it refuses while a native build, another game server, OBS or a busy GPU is
# seen (-Force skips that check), and holds C:\ai-bridge\heavy.lock for the measurement only.
param([int]$Runs = 3, [int]$Seconds = 30, [int]$Count = 50, [int]$Settle = 10, [int]$Crowd = 150, [int]$Herd = 450,
    [ValidateSet('revenants', 'mixed', 'zombies', 'empty', 'crowd')][string[]]$Cases = @('revenants', 'mixed', 'zombies', 'empty', 'crowd'),
    [string]$Engine = '', [string]$Label = '', [switch]$Force)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
Use-P5Server
$World = Join-Path $PSScriptRoot 'p5-world.ps1'
$Server = "F:\horizon-servers\soulscraft-p5"
$Lock = 'C:\ai-bridge\heavy.lock'
$Out = Join-Path $PSScriptRoot ("logs\p5-{0}{1}.txt" -f (Get-Date -Format 'yyyy-MM-dd-HHmm'), $(if ($Label) { "-$Label" } else { '' }))
$cx = -499.5; $cy = 101; $cz = 500.5
$Inv = [Globalization.CultureInfo]::InvariantCulture

function Assert-QuietPc {
    $busy = @()
    $names = 'cargo', 'rustc', 'horizon-launcher', 'horizon-mixins', 'link', 'obs64', 'obs32', 'horizon-engine'
    foreach ($p in Get-Process -ErrorAction SilentlyContinue | Where-Object { $names -contains $_.ProcessName }) {
        $busy += "$($p.ProcessName) $($p.Id)"
    }
    $gpu = & nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits 2>$null
    if ($gpu -and [int]($gpu | Select-Object -First 1) -gt 15) { $busy += "GPU at $gpu %" }
    if (Test-Path $Lock) { $busy += "heavy.lock: $(Get-Content $Lock -Raw)" }
    if (Test-Path "$env:LOCALAPPDATA\Horizon\joint-dev\maintenance.txt") { $busy += 'bench maintenance' }
    # The average of five seconds: a single reading catches a browser's spike as a busy PC.
    $cpu = [math]::Round((1..5 | ForEach-Object {
                (Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor -Filter "Name='_Total'").PercentProcessorTime
                Start-Sleep -Seconds 1
            } | Measure-Object -Average).Average, 1)
    if ($cpu -gt 20) { $busy += "CPU at $cpu %" }
    "quiet check $(Get-Date -Format s): CPU $cpu %, GPU $gpu %, busy: $(if ($busy) { $busy -join '; ' } else { 'nothing' })" |
        Tee-Object -FilePath $Out -Append | Write-Host
    if ($busy -and -not $Force) { throw "The PC is not quiet: $($busy -join '; ')" }
}

function Get-BuildProcesses {
    Get-Process -ErrorAction SilentlyContinue | Where-Object { 'cargo', 'rustc', 'horizon-launcher', 'horizon-mixins', 'link' -contains $_.ProcessName } |
        ForEach-Object { $_.ProcessName } | Sort-Object -Unique
}

function Get-Number([string]$Line, [string]$Pattern) {
    $m = [regex]::Match($Line, $Pattern)
    if ($m.Success) { [double]::Parse($m.Groups[1].Value, $Inv) } else { $null }
}

function Measure-Case([string]$Case, [scriptblock]$Spawn) {
    Invoke-Console 'sc clear' | Out-Null
    Invoke-Console 'kill @e[type=!minecraft:player]' | Out-Null
    & $Spawn
    Start-Sleep -Seconds $Settle
    $log = "$Server\logs\latest.log"
    $since = (Get-Content $log).Count
    # joint.ps1 does not wait for heavy.lock: a build starting during the sample marks it noisy.
    $before = Get-BuildProcesses
    Invoke-Console 'sc cost start' | Out-Null
    Start-Sleep -Seconds $Seconds
    Invoke-Console 'sc cost stop' | Out-Null
    $noisy = @($before) + @(Get-BuildProcesses) | Sort-Object -Unique
    Start-Sleep -Seconds 1
    $lines = Get-Content $log | Select-Object -Skip $since | Select-String 'soulscraft: cost ' |
        ForEach-Object { $_.Line -replace '^.*soulscraft: ', '' }
    $row = [ordered]@{ case = $Case; tick_ms = $null; own_ms = $null; mobs_ms = 0.0; kinds = @{}; parts = @{}; noisy = ($noisy -join ' ') }
    if ($row.noisy) { Add-Content $Out "  $Case | NOISY: $($row.noisy) ran during the sample" }
    foreach ($l in $lines) {
        Add-Content $Out "  $Case | $l"
        if ($l -match '^cost over') { $row.tick_ms = Get-Number $l 'average ([\d.]+) ms' }
        elseif ($l -match '^cost soulscraft_level_tick') { $row.own_ms = Get-Number $l 'ms_per_server_tick=([\d.]+)' }
        elseif ($l -match '^cost soulscraft_part part=(\S+)') { $row.parts[$Matches[1]] = Get-Number $l 'ms_per_server_tick=([\d.]+)' }
        elseif ($l -match '^cost kind=(\S+)') {
            $kind = $Matches[1]
            $ms = Get-Number $l 'ms_per_server_tick=([\d.]+)'
            $row.kinds[$kind] = $ms
            $row.mobs_ms += $ms
        }
    }
    [pscustomobject]$row
}

function Invoke-Run([int]$N) {
    & $World start -Engine $Engine | Tee-Object -FilePath $Out -Append | Write-Host
    $bot = $null
    try {
        $bot = Start-Bot sc_a
        # A client the server turns away (its content older than the server's: `joint.ps1 content`)
        # leaves the mobs with nobody to fight, and the numbers mean nothing.
        Start-Sleep -Seconds 5
        $list = (Invoke-Console 'list') -join ' '
        if ($list -notmatch '\bsc_a\b') { throw "The bot sc_a is not on the P5 server ($list): see its latest.log" }
        Invoke-Console 'sc arena' | Out-Null
        Invoke-Console "tp sc_a $cx $cy $cz 0 0" | Out-Null
        Invoke-Console 'gamemode survival sc_a' | Out-Null
        Invoke-Console 'effect give sc_a minecraft:resistance infinite 4 true' | Out-Null
        Invoke-Console 'effect give sc_a minecraft:saturation infinite 0 true' | Out-Null
        Invoke-Console 'sc p 0.5' | Out-Null
        # relic.rs reads the off hand twice a second: +4 max health with the Abyss Heart, back after.
        Invoke-Console 'item replace entity sc_a weapon.offhand with soulscraft:abyss_heart' | Out-Null
        Start-Sleep -Milliseconds 1500
        $held = (Invoke-Console 'attribute sc_a minecraft:max_health get') -join ' '
        Invoke-Console 'item replace entity sc_a weapon.offhand with minecraft:air' | Out-Null
        Start-Sleep -Milliseconds 1500
        $bare = (Invoke-Console 'attribute sc_a minecraft:max_health get') -join ' '
        "  relic | heart in the off hand: $held | hand empty again: $bare" | Tee-Object -FilePath $Out -Append | Write-Host
        $zombies = {
            for ($i = 0; $i -lt $Count; $i++) {
                $a = $i * 2 * [math]::PI / $Count
                $r = 8 + ($i % 3) * 3
                $x = ($cx + $r * [math]::Sin($a)).ToString('F1', $Inv)
                $z = ($cz + $r * [math]::Cos($a)).ToString('F1', $Inv)
                Invoke-Console "summon minecraft:zombie $x $cy $z {PersistenceRequired:1b}" | Out-Null
            }
        }
        # Monsters on rings 6..20 blocks out, cows on rings 24..60: one block apart or more, so the
        # engine's collisions stay what a herd in a field costs.
        $crowdSpawn = {
            $kinds = 'minecraft:zombie', 'minecraft:skeleton', 'minecraft:spider'
            for ($i = 0; $i -lt $Crowd; $i++) {
                $a = $i * 2.399963
                $r = 6 + 14 * [math]::Sqrt(($i + 0.5) / $Crowd)
                $x = ($cx + $r * [math]::Sin($a)).ToString('F1', $Inv)
                $z = ($cz + $r * [math]::Cos($a)).ToString('F1', $Inv)
                Invoke-Console "summon $($kinds[$i % 3]) $x $cy $z {PersistenceRequired:1b}" | Out-Null
            }
            for ($i = 0; $i -lt $Herd; $i++) {
                $a = $i * 2.399963
                $r = 24 + 36 * [math]::Sqrt(($i + 0.5) / $Herd)
                $x = ($cx + $r * [math]::Sin($a)).ToString('F1', $Inv)
                $z = ($cz + $r * [math]::Cos($a)).ToString('F1', $Inv)
                Invoke-Console "summon minecraft:cow $x $cy $z {PersistenceRequired:1b}" | Out-Null
            }
        }
        $mixed = 7 * [math]::Floor($Count / 7)
        $spawns = [ordered]@{
            revenants = { Invoke-Console "sc bench $Count" | Out-Null }
            mixed = { Invoke-Console "sc bench $mixed mixed" | Out-Null }
            zombies = $zombies
            empty = { }
            crowd = $crowdSpawn
        }
        @(foreach ($case in $spawns.Keys) {
                if ($Cases -contains $case) { Measure-Case $case $spawns[$case] }
            }) | ForEach-Object { $_ | Add-Member run $N -PassThru }
    } finally {
        if ($bot) { Stop-Bot $bot }
        & $World stop | Out-Null
    }
}

function Format-Stat($Values) {
    $v = @($Values | Where-Object { $null -ne $_ } | Sort-Object)
    if ($v.Count -eq 0) { return 'n/a' }
    $mid = [int][math]::Floor($v.Count / 2)
    $med = if ($v.Count % 2) { $v[$mid] } else { ($v[$mid - 1] + $v[$mid]) / 2 }
    '{0:F3} ({1:F3}..{2:F3})' -f $med, $v[0], $v[-1]
}

function Show-Stats($Rows) {
    $cases = @('revenants', 'mixed', 'zombies', 'empty', 'crowd' | Where-Object { $Cases -contains $_ })
    $table = foreach ($case in $cases) {
        $r = @($Rows | Where-Object { $_.case -eq $case -and -not $_.noisy })
        [pscustomobject]@{
            case = $case
            'server tick, ms' = Format-Stat $r.tick_ms
            'SoulsCraft hook, ms/tick' = Format-Stat $r.own_ms
            'mob ticks, ms/tick' = Format-Stat $r.mobs_ms
            'quiet samples' = "$($r.Count) of $(@($Rows | Where-Object case -eq $case).Count)"
        }
    }
    $kinds = foreach ($k in ($Rows | ForEach-Object { $_.kinds.Keys } | Sort-Object -Unique)) {
        foreach ($case in $cases) {
            $v = @($Rows | Where-Object { $_.case -eq $case -and -not $_.noisy } | ForEach-Object { $_.kinds[$k] } | Where-Object { $null -ne $_ })
            if ($v.Count) { [pscustomobject]@{ kind = $k; case = $case; 'ms per server tick' = Format-Stat $v } }
        }
    }
    $parts = foreach ($k in ($Rows | ForEach-Object { $_.parts.Keys } | Sort-Object -Unique)) {
        foreach ($case in $cases) {
            $v = @($Rows | Where-Object { $_.case -eq $case -and -not $_.noisy } | ForEach-Object { $_.parts[$k] } | Where-Object { $null -ne $_ })
            if ($v.Count) { [pscustomobject]@{ part = $k; case = $case; 'SoulsCraft ms per server tick' = Format-Stat $v } }
        }
    }
    ($table | Format-Table -AutoSize | Out-String -Width 200) + ($parts | Format-Table -AutoSize | Out-String -Width 200) +
    ($kinds | Format-Table -AutoSize | Out-String -Width 200) | Tee-Object -FilePath $Out -Append | Write-Host
}

New-Item -ItemType Directory -Force (Split-Path $Out) | Out-Null
Assert-QuietPc
Set-Content $Lock "soulscraft agent: П5 measurement (prototypes/p5-bench.ps1), $Runs runs, started $(Get-Date -Format s)"
$rows = @()
try {
    for ($n = 1; $n -le $Runs; $n++) {
        "== run $n of $Runs, $(Get-Date -Format s)" | Tee-Object -FilePath $Out -Append | Write-Host
        $rows += Invoke-Run $n
    }
} finally {
    Remove-Item -LiteralPath $Lock -ErrorAction SilentlyContinue
}
Show-Stats $rows
Write-Host "log: $Out"

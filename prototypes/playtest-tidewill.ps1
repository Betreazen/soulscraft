# Playtest on the live server tidewill (owner, 2026-11-16 03:12): SoulsCraft's mechanics on real
# terrain with the whole pack, and what they cost the server. Bot sc_a, SoulsCraft's area
# x -1000..0, z 0..1000 (RustCraft plays x 0..1000 at the same time).
#   preflight  the server's state, who is online, a backup before the world is touched
#   events     altars of levels 1..4 built on the ground, the bot's horn on each pedestal, the
#              event run to its end with nobody defending: waves on real terrain, the path map's
#              build time, breaches, curses, the altar's fate, the blocks put back; the server's
#              MSPT and SoulsCraft's own time during each event
#   night      only at night by the server's clock (the playtest never sets the time: RustCraft
#              shares the world): the bot stands in the open under Resistance V, what spawns
#   idle       the same numbers with no event, for comparison
# With -Perf the run holds %LOCALAPPDATA%\Horizon\joint-dev\perf-slot.txt (one measurement at a
# time across sessions) and also takes the bot client's frame stats.
#   pwsh -Command "& .\prototypes\playtest-tidewill.ps1 -Instance <pack-bots instance> -Only preflight,events"
param([Parameter(Mandatory)][string]$Instance, [string[]]$Only, [int[]]$Levels = @(1, 2, 3, 4),
    [int]$EventMinutes = 10, [int]$NightMinutes = 4, [switch]$Perf)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
. (Join-Path $PSScriptRoot 'lib\tidewill.ps1')
Use-Tidewill -Instance $Instance
$Stamp = Get-Date -Format 'yyyy-MM-dd-HHmm'
$Out = Join-Path $PSScriptRoot "evidence\tidewill\$Stamp"
New-Item -ItemType Directory -Force $Out | Out-Null
$Text = Join-Path $Out 'playtest.txt'
$Slot = "$env:LOCALAPPDATA\Horizon\joint-dev\perf-slot.txt"
$Mobs = 'revenant', 'stalker', 'marksman', 'brute', 'shrike', 'lurker', 'bog_slime'
$Horns = @{ 1 = 'soulscraft:rotten_horn'; 2 = 'soulscraft:grave_horn'; 3 = 'soulscraft:storm_horn'; 4 = 'soulscraft:abyss_horn' }
# Altar sites, one per level, 60 blocks apart so one event's zone (3 x 3 chunks) never holds another altar.
$Sites = @{ 1 = @(-620, 380); 2 = @(-560, 380); 3 = @(-500, 380); 4 = @(-440, 380) }
$NightSite = @(-300, 700)
# The scene marks and SoulsCraft's lines of the log, filtered on the server.
$LogGrep = 'grep 40000 soulscraft-playtest|soulscraft: '

function Scene([string]$Name) { -not $Only -or $Only -contains $Name }
function Note([string]$Title, $Lines) {
    Add-Content $Text "== $Title"
    @($Lines) | ForEach-Object { Add-Content $Text "  $_" }
    Write-Host "== $Title"
}
function Strip($l) { "$l" -replace '^\[[^\]]*\] \[[^\]]*\]: ', '' }

# The server's numbers from `status --json` (TPS, MSPT, players).
function Read-Status($Lines) {
    try { ($Lines -join "`n") | ConvertFrom-Json } catch { [pscustomobject]@{ raw = ($Lines -join ' ') } }
}
# A unique line in the server log: what came after it belongs to the scene.
function New-Mark([string]$Name) {
    $m = "soulscraft-playtest-$Name-$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())"
    Invoke-Remote @("cmd say $m") | Out-Null
    $m
}
function Since($Log, [string]$Mark) {
    $after = $false
    foreach ($l in $Log) {
        if ("$l" -match [regex]::Escape($Mark)) { $after = $true; continue }
        if ($after -and "$l" -match 'soulscraft: ') { Strip $l }
    }
}
# SoulsCraft's mobs near a point, and the vanilla zombies of the test event.
function Clear-Near([int]$X, [int]$Y, [int]$Z) {
    $steps = @($Mobs | ForEach-Object { "cmd kill @e[type=soulscraft:$_,x=$X,y=$Y,z=$Z,distance=..64]" })
    $steps += "cmd kill @e[type=minecraft:zombie,x=$X,y=$Y,z=$Z,distance=..64]"
    $steps += "cmd kill @e[type=minecraft:item,x=$X,y=$Y,z=$Z,distance=..24]"
    Invoke-Remote $steps | Out-Null
}
# The altar as `/sc altar` builds it (altar_ui.rs build_here), at a point on the ground.
function Altar-Steps([int[]]$Core, [int]$Level) {
    $x, $y, $z = $Core
    $r = 5
    $s = @("cmd fill $($x-$r) $($y-1) $($z-$r) $($x+$r) $($y-1) $($z+$r) stone",
           "cmd fill $($x-$r) $y $($z-$r) $($x+$r) $($y+4) $($z+$r) air",
           "cmd setblock $x $y $z soulscraft:altar_core",
           "cmd setblock $x $($y+1) $z soulscraft:altar_pedestal")
    $sockets = @{ 2 = @('-2,0', '2,0'); 3 = @('0,-3', '0,3'); 4 = @('2,-4', '4,2', '-2,4', '-4,-2') }
    $corner = @{ 2 = 'altar_pillar'; 3 = 'altar_brazier'; 4 = 'altar_obelisk' }
    for ($ring = 1; $ring -le $Level; $ring++) {
        for ($i = -$ring; $i -le $ring; $i++) {
            for ($k = -$ring; $k -le $ring; $k++) {
                if ([Math]::Max([Math]::Abs($i), [Math]::Abs($k)) -ne $ring) { continue }
                $block = if ($sockets[$ring] -contains "$i,$k") { 'altar_socket' } else { 'altar_slab' }
                $s += "cmd setblock $($x+$i) $y $($z+$k) soulscraft:$block"
            }
        }
        if ($corner[$ring]) {
            foreach ($c in @(@(-$ring, -$ring), @($ring, -$ring), @(-$ring, $ring), @($ring, $ring))) {
                $s += "cmd setblock $($x+$c[0]) $($y+1) $($z+$c[1]) soulscraft:$($corner[$ring])"
            }
        }
    }
    $s
}
# The bot onto the ground at x, z: up high, then it falls (creative takes no fall damage; the
# chunk loads first). `spreadplayers` did not move it on tidewill. Returns where it landed.
function Drop-At([int]$X, [int]$Z) {
    # Ground only: a landing on leaves, logs or water moves 7 blocks east and tries again (the
    # first playtest put a tier-1 altar on a tree top, and its waves were all cursed).
    for ($try = 0; $try -lt 8; $try++) {
        $tx = $X + 7 * $try
        $answer = (Invoke-Remote @("cmd tp sc_a $tx.5 200 $Z.5"))[0] -join ' '
        $last = $null
        $p = $null
        for ($i = 0; $i -lt 40; $i++) {
            Start-Sleep -Seconds 1
            $s = @((Invoke-Bot $bot 'state' | ConvertFrom-Json).player.position)
            if ($last -and [Math]::Abs($s[1] - $last[1]) -lt 0.01 -and $s[1] -lt 199) { $p = $s; break }
            $last = $s
        }
        if (-not $p) { throw "sc_a did not land at $tx,$Z (tp: $answer; at $($last -join ','))" }
        $under = (Invoke-Bot $bot "block $([Math]::Floor($p[0])) $([Math]::Floor($p[1]) - 1) $([Math]::Floor($p[2]))") -join ' '
        if ($under -notmatch 'leaves|_log|water|lava|ice|vine') { return $p }
        Note "landed on $(($under -replace '\s+', ' ').Substring(0, [Math]::Min(120, $under.Length))) at $($p -join ','): moving on" @()
    }
    throw "no open ground near $X,$Z"
}
function Take-Slot {
    if (Test-Path $Slot) { throw "perf slot is held: $(Get-Content $Slot -Raw)" }
    Set-Content $Slot "soulscraft session, $(Get-Date -Format s), playtest-tidewill"
}
function Drop-Slot { if ((Test-Path $Slot) -and (Get-Content $Slot -Raw) -match 'soulscraft') { Remove-Item $Slot } }

# Samples while something runs: every $Every seconds the status and the log tail, until $Until
# says stop (it gets the scene's SoulsCraft lines) or $Minutes pass.
function Watch([string]$Mark, [int]$Minutes, [scriptblock]$Until, [int]$Every = 20) {
    $samples = @()
    $lines = @()
    $end = (Get-Date).AddMinutes($Minutes)
    while ((Get-Date) -lt $end) {
        Start-Sleep -Seconds $Every
        $r = Invoke-Remote @('status --json', $LogGrep)
        $st = Read-Status $r[0]
        $samples += [pscustomobject]@{ at = Get-Date -Format 'HH:mm:ss'; status = $st }
        $lines = @(Since $r[1] $Mark)
        if ($Until -and (& $Until $lines)) { break }
    }
    [pscustomobject]@{ samples = $samples; lines = $lines }
}
# A full-size shot as a 960-wide JPEG for the repo.
function Save-Jpeg([string]$Png, [string]$Jpg) {
    if (-not (Test-Path $Png)) { return }
    $ps = "Add-Type -AssemblyName System.Drawing; `$i=[Drawing.Image]::FromFile('$Png'); `$h=[int](`$i.Height*960/`$i.Width); " +
        "`$b=New-Object Drawing.Bitmap `$i, 960, `$h; `$c=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders()|?{`$_.MimeType -eq 'image/jpeg'}; " +
        "`$p=New-Object Drawing.Imaging.EncoderParameters 1; `$p.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::Quality, [long]85); " +
        "`$b.Save('$Jpg', `$c, `$p); `$b.Dispose(); `$i.Dispose()"
    powershell.exe -NoProfile -Command $ps
}
function Summary-Of($Watch) {
    foreach ($s in $Watch.samples) {
        $g = $s.status.sample.game
        if (-not $g) { "$($s.at) $($s.status.raw)"; continue }
        $entities = (@($g.levels | ForEach-Object { $_.entities }) | Measure-Object -Sum).Sum
        '{0} mspt avg {1:N2} p95 {2:N2} p99 {3:N2}, tps {4:N1}, entities {5}, chunks {6}, players {7}, cpu {8:N0}%' -f `
            $s.at, $g.mspt.average, $g.mspt.p95, $g.mspt.p99, $g.tps, $entities, $g.chunks, @($g.players).Count, $s.status.sample.cpu_percent
    }
}

Set-Content $Text "SoulsCraft playtest on tidewill, $(Get-Date -Format s)"
$r = Invoke-Remote @('status --json', 'cmd list')
# The compact line only: the raw status names the players' addresses, which stay out of the repo.
Note 'server before' (@(Summary-Of ([pscustomobject]@{ samples = @([pscustomobject]@{ at = Get-Date -Format 'HH:mm:ss'; status = Read-Status $r[0] }) })) + @($r[1]))
$bot = $null
try {
    if (Scene 'preflight') {
        $b = Invoke-Remote @("backup soulscraft-playtest-$Stamp")
        Note 'backup' $b[0]
    }
    $bot = Start-Bot sc_a
    if ((Invoke-Bot $bot 'state' | ConvertFrom-Json).screen -match 'dead|death') { Invoke-Bot $bot 'close' | Out-Null; Start-Sleep -Seconds 2 }
    Invoke-Remote @('cmd sc log on', 'cmd gamemode creative sc_a', 'cmd effect give sc_a minecraft:resistance infinite 4 true',
        'cmd effect give sc_a minecraft:saturation infinite 0 true', 'cmd sc info') | ForEach-Object { $_ } | Out-Null
    if ($Perf) { Take-Slot }

    if (Scene 'idle') {
        $x, $z = $Sites[1]
        Drop-At $x $z | Out-Null
        $mark = New-Mark 'idle'
        Invoke-Remote @('cmd sc cost on') | Out-Null
        $fps = if ($Perf) { Invoke-Bot $bot 'bench 30' 90 } else { Start-Sleep -Seconds 30; $null }
        Invoke-Remote @('cmd sc cost off') | Out-Null
        $w = Watch $mark 1 { $true } 2
        Note 'idle: no event, bot standing' (@(Summary-Of $w) + @($w.lines | Where-Object { $_ -match 'cost' }) + @("client $fps"))
    }

    if (Scene 'events') {
        foreach ($level in $Levels) {
            $x, $z = $Sites[$level]
            $p = Drop-At $x $z
            # Each element in brackets: the comma binds tighter than `+`.
            $core = @([int][Math]::Floor($p[0]), [int][Math]::Floor($p[1]), ([int][Math]::Floor($p[2]) + 8))
            Note "bot on the ground at $($p -join ','), altar core $($core -join ',')" @()
            Invoke-Remote (Altar-Steps $core $level) | Out-Null
            $mark = New-Mark "event$level"
            Invoke-Remote @('cmd clear sc_a', "cmd give sc_a $($Horns[$level]) 1",
                "cmd tp sc_a $($core[0] + 0.5) $($core[1]) $($core[2] - 2.5) 0 30", 'cmd sc cost on') | Out-Null
            Start-Sleep -Milliseconds 800
            Invoke-Bot $bot "select $($Horns[$level])" | Out-Null
            Start-Sleep -Milliseconds 400
            Invoke-Bot $bot "interact $($core[0]) $($core[1] + 1) $($core[2])" | Out-Null
            # Out of the zone's middle, up high: the waves go for the altar, the creative bot watches.
            Start-Sleep -Seconds 3
            Invoke-Remote @("cmd tp sc_a $($core[0] - 12.5) $($core[1] + 10) $($core[2] - 12.5)") | Out-Null
            Invoke-Bot $bot "look-at $($core[0] + 0.5) $($core[1]) $($core[2] + 0.5)" | Out-Null
            $fps = $null
            if ($Perf) { Start-Sleep -Seconds 40; $fps = Invoke-Bot $bot 'bench 20' 60 }
            $shot = Join-Path $script:Scratch "tidewill-event$level.png"
            Invoke-Bot $bot "screenshot $shot" | Out-Null
            $w = Watch $mark $EventMinutes { param($l) @($l | Where-Object { $_ -match 'event (won|lost)' }).Count -gt 0 }
            Invoke-Remote @('cmd sc cost off') | Out-Null
            Start-Sleep -Seconds 3
            $tail = (Invoke-Remote @($LogGrep))[0]
            $lines = @(Since $tail $mark)
            $keep = $lines | Where-Object { $_ -match 'event (start|spawn|field|breach|curse|won|lost|reward|restored|not started|spawn skipped)|cost ' }
            Note "event level $level at $($core -join ',')" (@(Summary-Of $w) + @($keep) + @("client $fps"))
            $lines | Set-Content (Join-Path $Out "event$level.log")
            Save-Jpeg $shot (Join-Path $Out "event$level.jpg")
            Clear-Near $core[0] $core[1] $core[2]
            Invoke-Remote @("cmd fill $($core[0]-5) $($core[1]) $($core[2]-5) $($core[0]+5) $($core[1]+4) $($core[2]+5) air") | Out-Null
        }
    }

    if (Scene 'night') {
        $t = (Invoke-Remote @('cmd time query day'))[0] -join ' '
        # 26.3: "Timeline minecraft:day is at 1485 tick(s)".
        $day = if ($t -match 'is at (\d+)') { [int]$Matches[1] % 24000 } else { -1 }
        if ($day -lt 13000 -or $day -gt 22500) {
            Note 'night: skipped' "the server's daytime is $day (night is 13000..22500); the playtest never sets the time"
        } else {
            $x, $z = $NightSite
            Drop-At $x $z | Out-Null
            Invoke-Remote @('cmd gamemode survival sc_a') | Out-Null
            $mark = New-Mark 'night'
            Invoke-Remote @('cmd sc cost on') | Out-Null
            $w = Watch $mark $NightMinutes $null 30
            Invoke-Remote @('cmd sc cost off', 'cmd gamemode creative sc_a') | Out-Null
            Start-Sleep -Seconds 3
            $lines = @(Since ((Invoke-Remote @($LogGrep))[0]) $mark)
            # `adopt mob=` is SoulsCraft's own mob, `vadopt mob=` a vanilla monster the mod took over.
            $adopted = $lines | Where-Object { $_ -match ' (v?adopt)( moved)? mob=(\S+)' } | ForEach-Object { "$($Matches[1]) $($Matches[3])" } |
                Group-Object | Sort-Object Count -Descending | ForEach-Object { "$($_.Name) x$($_.Count)" }
            Note "night at $x,$z (daytime $day), $NightMinutes min" (@(Summary-Of $w) + @("adopted: $($adopted -join ', ')") + @($lines | Where-Object { $_ -match 'cost |natural|shard drop|world drop' }))
            $lines | Set-Content (Join-Path $Out 'night.log')
            Clear-Near $x 80 $z
        }
    }
} catch {
    Note 'error' @("$_", $_.ScriptStackTrace)
    throw
} finally {
    if ($Perf) { Drop-Slot }
    # Each step on its own: a dead ssh session must not keep the bot client running.
    try {
        Invoke-Remote @('cmd sc log off', 'cmd clear sc_a', 'cmd effect clear sc_a', 'cmd gamemode creative sc_a') | Out-Null
    } catch { Note 'cleanup on the server failed' "$_" }
    if ($bot) { Stop-Bot $bot }
    try {
        $st = Read-Status (Invoke-Remote @('status --json'))[0]
        Note 'server after' (Summary-Of ([pscustomobject]@{ samples = @([pscustomobject]@{ at = Get-Date -Format 'HH:mm:ss'; status = $st }) }))
    } catch { Note 'server after: no answer' "$_" }
    Close-Tidewill
}
Write-Host "Evidence in $Out"

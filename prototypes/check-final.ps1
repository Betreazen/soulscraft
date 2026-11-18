# SoulsCraft 0.7.0's new behaviour in game (docs/superpowers/plans/2026-11-16-final-version.md,
# tasks 1-3), bots sc_a (the target) and sc_b (the camera, a spectator with the GUI hidden) on the
# joint server. Scenes, each selectable with -Only:
#   revenant_hold   the revenant steps to its target during `hold`; the shield bash knocks back
#   zombie_lunge    a vanilla zombie's lunge stepped tick by tick: no step over dash_max_blocks_per_tick
#   stalker_flank   watched, it circles within reach and still strikes; looked away from, it strikes sooner
#   marksman_cover  a 3x3 stone wall 5 blocks off: after a shot the marksman stands behind it
#   brute_wave      overhead_wave on a standing bot (health lost) and a jumping one (none lost)
#   shrike_turns    three shrikes on one bot: their dives never overlap; one killed, shot falling
#   bog_slime       a shot mid-jump (the smoke shadow is in the picture); a size-4 slime's split
#   lurker_grab     in the arena pool: grab, three blows by the bot, freed
#   altar_menu      /sc altar 4, right-click on the core with an empty hand: the menu's shot
#   event_zone      the tier-2 event: zone outline and boss bar within 5 s, a wave shot, skeletons
#   event_bearer    the tier-1 event (rotten horn) up to wave 5: the banner-bearer looks big (~4 min)
#   record          a LetsCam recording of the tier-2 fight and its render to mp4 (no microphone)
# Evidence goes to prototypes/evidence/final/: JPEGs (960 wide), console.txt (what each scene
# measured) and the mp4 with its contact sheet. The run ends with a PASS/FAIL table; SEE-SHOT marks
# what only a person can read in a shot (a custom-drawn menu, particles). A failing scene is
# recorded and the script goes on; the cleanup always runs.
# Prerequisites: the joint server running offline with a client that has this build, and the
# LetsCam client mod in it (the record scene):
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"      (and stop, close after)
# Run: pwsh -Command "& .\prototypes\check-final.ps1"                       every scene
#      pwsh -Command "& .\prototypes\check-final.ps1 -Only brute_wave,lurker_grab"
# Wave-2 skeleton and grab/dive checks need the mobs of tasks 2 (wip/final-2) in the build; where a
# scene needs a log line the mod does not print (the shrike's token, the lurker's grab) it works from
# the phase/strike lines, positions and health, and says so in its reason.
param([string[]]$Only)
$ErrorActionPreference = 'Stop'
# The server's replies are UTF-8 (the boss bar's Russian name): read them so whatever shell started us.
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Repo = Split-Path $PSScriptRoot
$Out = Join-Path $PSScriptRoot 'evidence\final'
$Raw = Join-Path $script:Scratch 'final'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
$Text = Join-Path $Out 'console.txt'
$MobData = (Get-Content -Raw -Encoding utf8 (Join-Path $Repo 'data\mobs.json') | ConvertFrom-Json)
# The arena's middle, feet level (tools.rs ARENA is 100: the floor). `/sc altar` stands 6 south.
$cx = -499.5; $cy = 101; $cz = 500.5
$ax = -500; $ay = 101; $az = 506
$script:Results = [System.Collections.Generic.List[object]]::new()
$script:Converts = [System.Collections.Generic.List[object]]::new()
$script:Cam = $null
$script:Bot = $null

function Scene([string]$Name) { -not $Only -or $Only -contains $Name }
function C([string]$Line) { Invoke-Console $Line }
function V([string]$Status, [string]$Reason) { [pscustomobject]@{ Status = $Status; Reason = $Reason } }
function Note([string]$Title, $Lines) {
    Add-Content $Text "== $Title"
    @($Lines) | ForEach-Object { Add-Content $Text "  $_" }
}
function Mark { (Get-Content $log).Count }
function Get-LogSince([int]$Since, [string]$Pattern) {
    Get-Content $log | Select-Object -Skip $Since | Select-String $Pattern | ForEach-Object { $_.Line -replace '^\[[^\]]*\] \[[^\]]*\]: ', '' }
}
# The mod's `soulscraft: <kind> k=v …` lines after $Since as objects (lib\log.ps1's reader, but from a mark).
function Get-Rows([int]$Since, [string]$Kinds) {
    foreach ($line in @(Get-LogSince $Since "soulscraft: ($Kinds) ")) {
        if ($line -match 'soulscraft: (\w+) (.*)$') {
            $row = [ordered]@{ kind = $Matches[1] }
            foreach ($p in [regex]::Matches($Matches[2], '(\w+)=(\S+)')) { $row[$p.Groups[1].Value] = $p.Groups[2].Value }
            [pscustomobject]$row
        }
    }
}

# --- shots: the PNG is taken at once, the JPEG made later (Convert-Shots), so a timed series is not slowed ---
function Take-Shot($Bot, [string]$Name) {
    $png = Join-Path $Raw "$Name.png"
    Invoke-Bot $Bot "screenshot $png" | Out-Null
    $script:Converts.Add(@{ png = $png; jpg = (Join-Path $Out "$Name.jpg") })
}
function Convert-Shots {
    foreach ($s in $script:Converts) {
        if (-not (Test-Path $s.png)) { Write-Warning "no PNG $($s.png)"; continue }
        $ps = "Add-Type -AssemblyName System.Drawing; `$i=[Drawing.Image]::FromFile('$($s.png)'); `$w=960; `$h=[int](`$i.Height*`$w/`$i.Width); " +
            "`$b=New-Object Drawing.Bitmap `$i, `$w, `$h; `$c=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders()|?{`$_.MimeType -eq 'image/jpeg'}; " +
            "`$p=New-Object Drawing.Imaging.EncoderParameters 1; `$p.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::Quality, [long]85); " +
            "`$b.Save('$($s.jpg)', `$c, `$p); `$b.Dispose(); `$i.Dispose()"
        powershell.exe -NoProfile -Command $ps
        Write-Host "  shot $($s.jpg)"
    }
    $script:Converts.Clear()
}
function Save-Shot($Bot, [string]$Name) { Take-Shot $Bot $Name; Convert-Shots }

# --- world and bot helpers ---
function Get-Cam {
    if (-not $script:Cam) {
        $script:Cam = Start-Bot sc_b
        C 'gamemode spectator sc_b' | Out-Null
        Invoke-Bot $script:Cam 'key f1 tap' | Out-Null
    }
    $script:Cam
}
function Set-Survivor {
    C 'gamemode survival sc_a' | Out-Null
    C 'effect give sc_a minecraft:resistance infinite 4 true' | Out-Null
    C 'effect give sc_a minecraft:regeneration infinite 2 true' | Out-Null
}
function Reset-Scene {
    C 'tick unfreeze' | Out-Null
    C 'sc slowmo off' | Out-Null
    C 'sc event stop' | Out-Null
    C 'sc clear' | Out-Null
    C 'sc altar 0' | Out-Null
    C 'effect clear sc_a' | Out-Null
    C 'gamemode creative sc_a' | Out-Null
    C 'sc arena' | Out-Null
    C 'kill @e[type=minecraft:item,x=-500,y=101,z=500,distance=..40]' | Out-Null
    Start-Sleep -Seconds 2
    C "tp sc_a $cx $cy $($cz - 3) 0 12" | Out-Null
    Start-Sleep -Seconds 1
}
# A mob's position from the server: the nearest of the type to the arena (or to $Near).
function Get-MobPos([string]$Type, $Near = @($cx, $cy, $cz)) {
    $sel = "@e[type=$Type,limit=1,sort=nearest,x=$($Near[0]),y=$($Near[1]),z=$($Near[2])]"
    $r = (C "data get entity $sel Pos") -join ' '
    if ($r -match '\[(-?[\d.]+)d, (-?[\d.]+)d, (-?[\d.]+)d\]') { return @([double]$Matches[1], [double]$Matches[2], [double]$Matches[3]) }
    $null
}
function Get-BotState { Invoke-Bot $script:Bot 'state' | Out-String | ConvertFrom-Json }
function Get-BotPos { (Get-BotState).player.position }
function Get-Health { [double](Get-BotState).player.health }
function Dist2([double[]]$A, [double[]]$B) { [math]::Sqrt([math]::Pow($A[0] - $B[0], 2) + [math]::Pow($A[2] - $B[2], 2)) }
# Where $To lies off the view's axis ($Yaw: 0 south, 90 west), degrees 0..180.
function Off-Axis([double[]]$From, [double]$Yaw, [double[]]$To) {
    $b = [math]::Atan2(-($To[0] - $From[0]), ($To[2] - $From[2])) * 180 / [math]::PI
    [math]::Abs((($b - $Yaw + 540) % 360) - 180)
}
function Bearing([double[]]$From, [double[]]$To) { [math]::Atan2(-($To[0] - $From[0]), ($To[2] - $From[2])) * 180 / [math]::PI }
# `/tick step` returns at once and the ticks run after it: wait them out (shots.ps1).
function Step-World([int]$Ticks) {
    C "tick step $Ticks" | Out-Null
    Start-Sleep -Milliseconds ($Ticks * 50 + 1500)
}
function Get-Attack([string]$Mob, [string]$Attack) { ($MobData.mobs | Where-Object id -eq $Mob).attacks | Where-Object id -eq $Attack }
# Horn on the pedestal of the altar `sc altar` just built (check-survival.ps1's Pedestal, without its wait).
function Start-Event([string]$Horn) {
    C 'clear sc_a' | Out-Null
    C "give sc_a $Horn 1" | Out-Null
    C "tp sc_a $($ax + 0.5) $ay $($az - 2.5) 0 30" | Out-Null
    Start-Sleep -Milliseconds 600
    Invoke-Bot $script:Bot "select $Horn" | Out-Null
    Start-Sleep -Milliseconds 400
    Invoke-Bot $script:Bot "interact $ax $($ay + 1) $az" | Out-Null
}
# Kills the event's wave mobs until the log says `event spawn wave=$Wave`; false after $MaxSec.
function Skip-ToWave([int]$Wave, [int]$Since, [int]$MaxSec) {
    $t0 = Get-Date
    while (((Get-Date) - $t0).TotalSeconds -lt $MaxSec) {
        if (@(Get-LogSince $Since "soulscraft: event spawn wave=$Wave ").Count -gt 0) { return $true }
        C 'kill @e[tag=soulscraft.wave]' | Out-Null
        Start-Sleep -Seconds 3
    }
    $false
}

function Run-Scene([string]$Name, [scriptblock]$Body) {
    if (-not (Scene $Name)) { return }
    Write-Host "== $Name"
    Add-Content $Text "`n######## $Name, $(Get-Date -Format s)"
    $v = $null
    try {
        $v = @(& $Body) | Where-Object { $_ -and $_.PSObject.Properties.Name -contains 'Status' } | Select-Object -Last 1
        if (-not $v) { $v = V 'FAIL' 'the scene gave no verdict' }
    } catch {
        $v = V 'FAIL' "exception: $($_.Exception.Message)"
    }
    try { Convert-Shots } catch { Write-Warning "shots: $_" }
    try { Reset-Scene } catch { Write-Warning "reset after $Name`: $_" }
    Add-Content $Text "-> $($v.Status): $($v.Reason)"
    Write-Host "  $($v.Status): $($v.Reason)"
    $script:Results.Add([pscustomobject]@{ Scene = $Name; Status = $v.Status; Reason = $v.Reason })
}

Set-Content $Text "final-version checks (0.7.0), $(Get-Date -Format s)"
if (Scene 'record') {
    # LetsCam's test config (letscam/tools/lib.ps1): no microphone, the output on a known folder.
    # The client reads it at launch, so it is set before the bots start.
    $LcOut = if ($env:SOULSCRAFT_LETSCAM_OUT) { $env:SOULSCRAFT_LETSCAM_OUT } elseif (Test-Path 'F:\') { 'F:\letscam\joint-dev' } else { Join-Path $script:Scratch 'letscam' }
    $LcConfig = Join-Path $script:Instance 'config\letscam-client.toml'
    New-Item -ItemType Directory -Force (Split-Path $LcConfig), $LcOut | Out-Null
    $LcBackup = if (Test-Path $LcConfig) { Get-Content -Raw $LcConfig } else { $null }
    Set-Content -Encoding utf8 $LcConfig "record_mic = false`nkeep_sources = true`noutput_dir = `"$($LcOut -replace '\\', '/')`"`n"
}
$script:Bot = Start-Bot sc_a
try {
    foreach ($g in 'gamerule advance_time false', 'time set noon', 'weather clear') { C $g | Out-Null }
    Reset-Scene

    Run-Scene 'revenant_hold' {
        # Part 1, frozen and stepped (shots.ps1): the revenant's distance to the bot at the end of the
        # windup, in the middle of the hold and at the hit; with hold_step_speed it must shrink.
        Set-Survivor
        C 'sc spawn revenant 0 1' | Out-Null
        Start-Sleep -Seconds 2
        $mark = Mark
        $a = Get-Attack 'revenant' 'overhead'
        $bp = @($cx, $cy, ($cz - 3))
        C 'tick freeze' | Out-Null
        C 'sc force overhead' | Out-Null
        $stops = [ordered]@{ windup_end = $a.windup; hold_mid = $a.windup + [math]::Floor($a.hold / 2); hit = $a.windup + $a.hold }
        $at = -1; $dist = [ordered]@{}; $lines = @()
        foreach ($k in $stops.Keys) {
            Step-World ($stops[$k] - $at); $at = $stops[$k]
            $p = Get-MobPos 'soulscraft:revenant'
            $dist[$k] = Dist2 $p $bp
            $lines += "t=$at $k mob at $([math]::Round($p[0], 2)),$([math]::Round($p[2], 2)) distance to the bot $([math]::Round($dist[$k], 2))"
            Take-Shot $script:Bot "revenant_hold_$k"
        }
        C 'tick unfreeze' | Out-Null
        Start-Sleep -Seconds 1
        Note 'revenant hold: phase lines' (@(Get-LogSince $mark 'soulscraft: (phase|strike) mob=revenant') + $lines)
        $closed = $dist['windup_end'] - $dist['hit']
        $holdOk = $closed -ge 0.15
        # Part 2: the shield bash on a bot standing 1.6 blocks off; the bot's displacement.
        C 'sc clear' | Out-Null
        C 'sc spawn revenant 0 1' | Out-Null
        Start-Sleep -Seconds 2
        $m = Get-MobPos 'soulscraft:revenant'
        C "tp sc_a $($m[0]) $($m[1]) $($m[2] - 1.6) 0 5" | Out-Null
        Start-Sleep -Seconds 1
        $before = Get-BotPos
        $mark = Mark
        C 'sc force shield_bash' | Out-Null
        Start-Sleep -Milliseconds 1500
        $after = Get-BotPos
        $moved = Dist2 $before $after
        Take-Shot $script:Bot 'revenant_shield_bash_after'
        Note 'revenant shield bash' (@(Get-LogSince $mark 'soulscraft: (phase|strike) mob=revenant') + "bot moved $([math]::Round($moved, 2)) blocks (survival, resistance on)")
        if ($holdOk -and $moved -ge 1.0) { return V 'PASS' "closed $([math]::Round($closed, 2)) blocks during the hold; the bash threw the bot $([math]::Round($moved, 2)) blocks" }
        V 'FAIL' "hold closed $([math]::Round($closed, 2)) blocks (need >= 0.15); bash moved the bot $([math]::Round($moved, 2)) (need >= 1)"
    }

    Run-Scene 'zombie_lunge' {
        # Playtest 2026-11-18: a vanilla zombie's lunge looked like a teleport. The world is frozen and
        # stepped a tick at a time through the lunge; the zombie's step per tick must stay under
        # dash_max_blocks_per_tick (0.4) plus slack, and the whole lunge must move it.
        Set-Survivor
        $cap = [double]$MobData.dash_max_blocks_per_tick
        $lunge = $MobData.vanilla.groups.Where({ $_.id -eq 'zombie' })[0].attacks.Where({ $_.id -eq 'lunge' })[0]
        $span = [int]$lunge.windup + [int]$lunge.hit + 2
        C "tp sc_a $cx $cy $($cz - 3) 0 0" | Out-Null
        C "summon minecraft:zombie $cx $cy $($cz + 3)" | Out-Null
        Start-Sleep -Seconds 2
        C 'tick freeze' | Out-Null
        C 'sc force lunge' | Out-Null
        $steps = @(); $prev = Get-MobPos 'minecraft:zombie'; $start = $prev
        for ($i = 0; $i -lt $span; $i++) {
            C 'tick step 1' | Out-Null
            Start-Sleep -Milliseconds 120
            $now = Get-MobPos 'minecraft:zombie'
            if ($now -and $prev) { $steps += [math]::Round([math]::Sqrt([math]::Pow($now[0] - $prev[0], 2) + [math]::Pow($now[2] - $prev[2], 2)), 3) }
            if ($i -eq [int]$lunge.windup + 2) { Take-Shot $script:Bot 'zombie_lunge_mid' }
            $prev = $now
        }
        C 'tick unfreeze' | Out-Null
        $moved = if ($start -and $prev) { [math]::Round([math]::Sqrt([math]::Pow($prev[0] - $start[0], 2) + [math]::Pow($prev[2] - $start[2], 2)), 2) } else { 0 }
        $top = if ($steps.Count) { ($steps | Measure-Object -Maximum).Maximum } else { 0 }
        Note 'zombie lunge, step per tick' ("steps: $($steps -join ' '); moved $moved blocks; cap $cap")
        if ($steps.Count -lt $span - 2) { return V 'FAIL' "could not follow the zombie ($($steps.Count) samples)" }
        if ($moved -lt 1.0) { return V 'FAIL' "the lunge moved the zombie only $moved blocks (was it forced? see console.txt)" }
        if ($top -gt $cap + 0.1) { return V 'FAIL' "a step of $top blocks in one tick, over the cap ${cap}: still a jump" }
        V 'PASS' "fastest step $top blocks a tick (cap $cap), the lunge moved it $moved blocks"
    }

    Run-Scene 'stalker_flank' {
        # Trial A (0.7.1, playtest 2026-11-18): the bot keeps looking at the stalker for 4 s; it circles
        # within reach (<= 4.5 blocks) and still comes in to strike after flank_max_ticks (40). Trial B: a fresh stalker, the bot looks away (`look`, re-aimed every 0.4 s so the
        # stalker never gets into the view): it strikes before flank_max_ticks.
        Set-Survivor
        $bp = @($cx, $cy, ($cz - 3))
        $max = [int]$MobData.mobs.Where({ $_.id -eq 'stalker' })[0].params.flank_max_ticks
        if ($max -le 0) { $max = 100 }
        # A
        C 'sc spawn stalker 0 1' | Out-Null
        Start-Sleep -Seconds 1
        $markA = Mark
        $bearings = @(); $dists = @(); $t0 = Get-Date
        while (((Get-Date) - $t0).TotalSeconds -lt 4) {
            $m = Get-MobPos 'soulscraft:stalker'
            if ($m) {
                Invoke-Bot $script:Bot "look-at $($m[0]) $($m[1] + 1) $($m[2])" | Out-Null
                $bearings += [math]::Round((Bearing $bp $m), 0)
                $dists += [math]::Round([math]::Sqrt([math]::Pow($m[0] - $bp[0], 2) + [math]::Pow($m[2] - $bp[2], 2)), 2)
            }
        }
        Take-Shot $script:Bot 'stalker_watched'
        $rowsA = @(Get-Rows $markA 'mode|strike|phase')
        $strikesA = @($rowsA | Where-Object kind -eq 'strike').Count
        $span = if ($bearings.Count) { ($bearings | Measure-Object -Maximum -Minimum | ForEach-Object { $_.Maximum - $_.Minimum }) } else { 0 }
        Note 'stalker watched (4 s)' (@($rowsA | ForEach-Object { ($_.PSObject.Properties | ForEach-Object { "$($_.Name)=$($_.Value)" }) -join ' ' }) + "bearings from the bot: $($bearings -join ' ') (circled $span degrees); distances $($dists -join ' '); strikes $strikesA")
        # B
        C 'sc clear' | Out-Null
        C "tp sc_a $cx $cy $($cz - 3) 0 0" | Out-Null
        Start-Sleep -Seconds 1
        $markB = Mark
        C 'sc spawn stalker 0 1' | Out-Null
        $struck = $false; $offAtStrike = $null; $t0 = Get-Date
        while (((Get-Date) - $t0).TotalSeconds -lt 9 -and -not $struck) {
            $m = Get-MobPos 'soulscraft:stalker'
            if ($m) {
                $away = Bearing $bp $m
                $away = if ($away -gt 0) { $away - 180 } else { $away + 180 }
                Invoke-Bot $script:Bot "look $([math]::Round($away, 1)) 0" | Out-Null
                $offAtStrike = [math]::Round((Off-Axis $bp $away $m), 0)
            }
            $struck = @(Get-Rows $markB 'strike').Count -gt 0
        }
        Take-Shot $script:Bot 'stalker_unseen_strike'
        $rowsB = @(Get-Rows $markB 'adopt|mode|strike|phase')
        $adopt = $rowsB | Where-Object { $_.kind -eq 'adopt' -and $_.mob -eq 'stalker' } | Select-Object -First 1
        $strike = $rowsB | Where-Object kind -eq 'strike' | Select-Object -First 1
        $ticks = if ($adopt -and $strike) { [int64]$strike.tick - [int64]$adopt.tick } else { $null }
        Note 'stalker looked away from' (@($rowsB | ForEach-Object { ($_.PSObject.Properties | ForEach-Object { "$($_.Name)=$($_.Value)" }) -join ' ' }) + "strike $ticks ticks after adopt; stalker $offAtStrike degrees off the view axis (sees_me is true inside 60)")
        # Ring distance: the samples after the approach (the first second walks in from the spawn).
        $ring = @($dists | Select-Object -Skip ([math]::Floor($dists.Count / 4)))
        $far = if ($ring.Count) { ($ring | Measure-Object -Maximum).Maximum } else { 99 }
        if ($strikesA -lt 1) { return V 'FAIL' "watched for 4 s, it never came in to strike (flank_max_ticks $max)" }
        if ($far -gt 4.5) { return V 'FAIL' "watched, it circled as far as $far blocks, out of a sword's reach and a step" }
        if (-not $struck) { return V 'FAIL' 'never struck in 9 s with the bot looking away' }
        if ($ticks -ge $max) { return V 'FAIL' "struck $ticks ticks after adopt, not before flank_max_ticks $max (a timeout, not an unseen strike)" }
        V 'PASS' "watched: circled at most $far blocks off and struck $strikesA time(s) in 4 s; looked away from: struck $ticks ticks after adopt (< $max), $offAtStrike degrees off the view"
    }

    Run-Scene 'marksman_cover' {
        # Wall x -496..-494, y 101..103, z 502 (3x3), 5.4 blocks from the marksman at z 500.5; the
        # bot is its target 10 blocks north (in survival: a creative player is no target, and the
        # first re-run's forced shot went off at nobody). After the shot the marksman reloads: behind the wall means
        # its line to the bot crosses the wall's plane inside the wall and it stands past it.
        $cam = Get-Cam
        Set-Survivor
        C 'fill -496 101 502 -494 103 502 stone' | Out-Null
        C "tp sc_a $cx $cy $($cz - 5) 0 0" | Out-Null
        Start-Sleep -Seconds 1
        $markS = Mark
        C 'sc spawn marksman 0 1' | Out-Null
        Start-Sleep -Seconds 2
        C "tp sc_a $cx $cy $($cz - 10) 0 0" | Out-Null
        C "tp sc_b -494.5 118 497 0 90" | Out-Null
        Start-Sleep -Seconds 2
        Take-Shot $cam 'marksman_cover_before'
        $mark = Mark
        C 'sc force aimed_shot' | Out-Null
        $start = Get-Date; $m = $null; $cover = @()
        while (((Get-Date) - $start).TotalSeconds -lt 6) {
            Start-Sleep -Milliseconds 700
            $m = Get-MobPos 'soulscraft:marksman'
            $cover = @(Get-LogSince $mark 'soulscraft: (reload_cover|cover) ')
            if ($m -and $m[2] -gt 503.2) { break }
        }
        Take-Shot $cam 'marksman_cover_after'
        $bp = @($cx, $cy, ($cz - 10))
        $behind = $false
        if ($m -and $m[2] -gt 503.0) {
            $xAtWall = $bp[0] + ($m[0] - $bp[0]) * (502.5 - $bp[2]) / ($m[2] - $bp[2])
            $behind = $xAtWall -ge -496.3 -and $xAtWall -le -493.7
        }
        Note 'marksman cover' (@(Get-LogSince $markS 'soulscraft: (adopt|phase|strike|reload_cover|cover) ') + "marksman at $(if ($m) { ($m | ForEach-Object { [math]::Round($_, 2) }) -join ',' } else { 'unknown' }); behind the wall: $behind")
        C 'fill -496 101 502 -494 103 502 air' | Out-Null
        if (-not $m) { return V 'FAIL' 'no marksman found after the shot' }
        if ($behind) { return V 'PASS' "marksman at $([math]::Round($m[0], 1)),$([math]::Round($m[2], 1)) is behind the wall ($(@($cover).Count) cover log line(s))" }
        V 'FAIL' "marksman at $([math]::Round($m[0], 1)),$([math]::Round($m[2], 1)) after 6 s, not behind the wall ($(@($cover).Count) cover log line(s))"
    }

    Run-Scene 'brute_wave' {
        # overhead_wave (windup 16, the blow at t 16) with the bot 2 blocks in front of the brute, the
        # impact point. The world is frozen and stepped (shots.ps1), so the jump is timed in ticks,
        # not by the wall clock: the attack is stepped to t = $pre - 1, the bot holds space (players
        # move while the world is frozen), then the world steps on through the blow, which falls
        # 16 - $pre + 1 ticks (about 150-200 ms) into the jump, the feet about 0.7-1 block up.
        # Standing: health must drop. Jumping: none lost, and the strike line says `jumped=true`
        # (server.rs deal: the target over a jumpable wave is over the hammer too, 0.7.0 bench fix).
        $trial = {
            param([bool]$Jump, [int]$Pre)
            C 'tick unfreeze' | Out-Null
            C 'sc clear' | Out-Null
            if ((Get-Health) -le 0) { Invoke-Bot $script:Bot 'respawn' | Out-Null; Start-Sleep -Seconds 3 }
            C 'gamemode survival sc_a' | Out-Null
            C 'effect clear sc_a' | Out-Null
            C 'effect give sc_a minecraft:instant_health 1 3 true' | Out-Null
            C 'effect give sc_a minecraft:resistance 60 1 true' | Out-Null
            C "tp sc_a $cx $cy $($cz - 3) 0 0" | Out-Null
            C 'sc spawn brute 0 1' | Out-Null
            Start-Sleep -Seconds 2
            $b = Get-MobPos 'soulscraft:brute'
            C "tp sc_a $($b[0]) $($b[1]) $($b[2] - 2.0) 0 0" | Out-Null
            Start-Sleep -Seconds 2
            $h0 = Get-Health
            $mark = Mark
            C 'tick freeze' | Out-Null
            C 'sc force overhead_wave' | Out-Null
            Step-World $Pre
            $ys = @()
            if ($Jump) { Invoke-Bot $script:Bot 'key space down' | Out-Null }
            C 'tick step 8' | Out-Null
            for ($i = 0; $i -lt 4; $i++) { $ys += [math]::Round([double]((Get-BotPos)[1]) - $b[1], 2); Start-Sleep -Milliseconds 40 }
            if ($Jump) { Invoke-Bot $script:Bot 'key space up' | Out-Null }
            Step-World 30
            C 'tick unfreeze' | Out-Null
            Start-Sleep -Milliseconds 500
            $h1 = Get-Health
            $lines = @(Get-LogSince $mark 'soulscraft: (phase|strike) mob=brute')
            [pscustomobject]@{ lost = [math]::Round($h0 - $h1, 2); lines = $lines; ys = ($ys -join ' '); jumped = @($lines | Where-Object { $_ -match 'jumped=true' }).Count -gt 0 }
        }
        $stand = & $trial $false 14
        Note 'brute standing' (@($stand.lines) + "feet over the floor after the step: $($stand.ys); health lost $($stand.lost)")
        Take-Shot $script:Bot 'brute_wave_standing_after'
        $jumps = @()
        foreach ($pre in 14, 13) {
            $j = & $trial $true $pre
            $jumps += [pscustomobject]@{ pre = $pre; lost = $j.lost; jumped = $j.jumped }
            Note "brute jumping (space held at t $($pre - 1), the blow $(17 - $pre) ticks later)" (@($j.lines) + "feet over the floor just after the step: $($j.ys); health lost $($j.lost)")
        }
        $dodged = @($jumps | Where-Object { $_.lost -le 0 })
        $summary = ($jumps | ForEach-Object { "t$($_.pre - 1):-$($_.lost)$(if ($_.jumped) { ' jumped' })" }) -join ' '
        if ($stand.lost -le 0) { return V 'FAIL' "the standing bot lost no health (wave did not hit; jumps $summary)" }
        if ($dodged.Count -eq 0) { return V 'FAIL' "standing lost $($stand.lost); no tick-timed jump dodged it ($summary): the hammer hits a jumping target (fixed in server.rs deal, needs the rebuilt bundle)" }
        V 'PASS' "standing lost $($stand.lost); jumping $summary ($($dodged.Count) of $($jumps.Count) tick-timed jumps took nothing)"
    }

    Run-Scene 'shrike_turns' {
        # Three shrikes, one survivor-bot under them for 25 s. A dive's span is its windup tick to
        # its `recover` phase tick (the dive itself): spans of different shrikes must not overlap.
        # The mod prints no token line, so the proof is the phase lines.
        $cam = Get-Cam
        Set-Survivor
        $mark = Mark
        C 'sc spawn shrike 0.3 3' | Out-Null
        Start-Sleep -Seconds 25
        $rows = @(Get-Rows $mark 'phase') | Where-Object { $_.mob -eq 'shrike' }
        $spans = @()
        foreach ($g in ($rows | Group-Object id)) {
            $w = $null
            foreach ($r in $g.Group) {
                if ($r.phase -eq 'windup') { $w = [int64]$r.tick }
                if ($r.phase -eq 'recover' -and $null -ne $w) { $spans += [pscustomobject]@{ id = $g.Name; from = $w; to = [int64]$r.tick }; $w = $null }
            }
        }
        $overlaps = @()
        foreach ($i in 0..([math]::Max($spans.Count - 1, 0))) {
            foreach ($j in ($i + 1)..([math]::Max($spans.Count - 1, 0))) {
                if ($j -lt $spans.Count -and $i -lt $spans.Count -and $spans[$i].id -ne $spans[$j].id -and $spans[$i].from -lt $spans[$j].to -and $spans[$j].from -lt $spans[$i].to) {
                    $overlaps += "ids $($spans[$i].id)/$($spans[$j].id): $($spans[$i].from)-$($spans[$i].to) and $($spans[$j].from)-$($spans[$j].to)"
                }
            }
        }
        $ids = @($spans | Select-Object -ExpandProperty id -Unique)
        Note 'shrike dives (id from..to ticks)' (@($spans | ForEach-Object { "id=$($_.id) $($_.from)..$($_.to)" }) + "overlaps: $($overlaps.Count)" + $overlaps)
        # Kill one and shoot it falling: frozen, stepped (shots.ps1).
        $mark2 = Mark
        $p0 = Get-MobPos 'soulscraft:shrike'
        $fall = $null
        if ($p0) {
            C "tp sc_b $($p0[0] + 5) $($p0[1] + 1) $($p0[2]) -90 0" | Out-Null
            Start-Sleep -Seconds 1
            Invoke-Bot $cam "look-at $($p0[0]) $($p0[1]) $($p0[2])" | Out-Null
            C 'tick freeze' | Out-Null
            C "damage @e[type=soulscraft:shrike,limit=1,sort=nearest,x=$($p0[0]),y=$($p0[1]),z=$($p0[2])] 1000 minecraft:player_attack by sc_a" | Out-Null
            Step-World 6
            Take-Shot $cam 'shrike_falling_t6'
            $p6 = Get-MobPos 'soulscraft:shrike' @($p0[0], $p0[1], $p0[2])
            Step-World 10
            Take-Shot $cam 'shrike_falling_t16'
            $p16 = Get-MobPos 'soulscraft:shrike' @($p0[0], $p0[1], $p0[2])
            C 'tick unfreeze' | Out-Null
            $fall = "Y at the kill $([math]::Round($p0[1], 2)), at t6 $(if ($p6) { [math]::Round($p6[1], 2) } else { 'gone' }), at t16 $(if ($p16) { [math]::Round($p16[1], 2) } else { 'gone' })"
        }
        Note 'shrike death' (@(Get-LogSince $mark2 'soulscraft: (death mob=shrike|dead flyer)') + $fall)
        if ($spans.Count -lt 3 -or $ids.Count -lt 2) { return V 'FAIL' "only $($spans.Count) dives by $($ids.Count) shrike(s) in 25 s: nothing to compare" }
        if ($overlaps.Count -gt 0) { return V 'FAIL' "$($overlaps.Count) overlapping dive(s): $($overlaps[0])" }
        V 'PASS' "$($spans.Count) dives by $($ids.Count) shrikes, none overlapping; fall: $fall (the falling shots are for the eye)"
    }

    Run-Scene 'bog_slime' {
        # A: frozen and stepped one tick at a time from a slime standing on the ground (one caught
        # mid-hop is stepped down first: a jump forced in the air starts from there, which gave the
        # first run's 7.5), `sc force high_jump`, its Y read every tick: the apex against vanilla
        # physics for sizes.4.jump_velocity (y += v; v = (v - 0.08) * 0.98: 0.8 gives ~3.97). The camera
        # stands 5 blocks east, 2 up (the slime is small: 0.8 block across at size 4), framing the slime's path and its landing spot, where the smoke
        # shadow is (for the eye). B: a size-4 slime killed by the bot, the size-2 slimes counted.
        $cam = Get-Cam
        Set-Survivor
        C "tp sc_b $($cx + 5) $($cy + 2) $($cz - 1.5) 90 0" | Out-Null
        C 'sc spawn bog_slime 0 1' | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $cam "look-at $cx $($cy + 2) $($cz - 1.5)" | Out-Null
        $sel = "@e[type=soulscraft:bog_slime,limit=1,sort=nearest,x=$cx,y=$cy,z=$cz]"
        C 'tick freeze' | Out-Null
        $settle = 0
        for ($i = 0; $i -lt 40 -and $settle -lt 3; $i++) {
            if (((C "data get entity $sel OnGround") -join ' ') -match '1b') { $settle++ } else { $settle = 0 }
            if ($settle -lt 3) { C 'tick step 1' | Out-Null; Start-Sleep -Milliseconds 150 }
        }
        $start = Get-MobPos 'soulscraft:bog_slime'
        $ground = $start[1]
        $camSt = (Invoke-Bot $cam 'state' | Out-String | ConvertFrom-Json).player
        $a = Get-Attack 'bog_slime' 'high_jump'
        $jv = [double](($MobData.mobs | Where-Object id -eq 'bog_slime').sizes | Where-Object size -eq 4).jump_velocity
        $want = 0.0; $v = $jv; while ($v -gt 0) { $want += $v; $v = ($v - 0.08) * 0.98 }
        $mark = Mark
        C 'sc force high_jump' | Out-Null
        $shots = @(3, 6, 10, 16) | ForEach-Object { [int]$a.windup + $_ }
        $top = $ground; $topAt = 0; $ys = @()
        for ($t = 1; $t -le $a.windup + 30; $t++) {
            C 'tick step 1' | Out-Null
            Start-Sleep -Milliseconds 150
            $p = Get-MobPos 'soulscraft:bog_slime'
            if ($p) { $ys += [math]::Round($p[1] - $ground, 2); if ($p[1] -gt $top) { $top = $p[1]; $topAt = $t } }
            if ($shots -contains $t) { Start-Sleep -Milliseconds 1200; Take-Shot $cam "bog_slime_midjump_t$t" }
            # The shadow is smoke every shadow_every_ticks (4): a shot right after each of 4 ticks in a row,
            # before the client lets the puffs fade, catches one.
            if ($t -ge $a.windup + 7 -and $t -le $a.windup + 10) { Take-Shot $cam "bog_slime_shadow_t$t" }
        }
        C 'tick unfreeze' | Out-Null
        $height = $top - $ground
        Note "bog slime jump (ground y $ground, jump_velocity $jv, vanilla apex $([math]::Round($want, 2)))" (@(Get-LogSince $mark 'soulscraft: (phase|strike) mob=bog_slime') + "height over the ground, tick by tick from the force: $($ys -join ' ')" + "apex $([math]::Round($height, 2)) at step $topAt" + "slime from $($start -join ' '); camera at $($camSt.position -join ' ') yaw $($camSt.yaw) pitch $($camSt.pitch)")
        C 'sc clear' | Out-Null
        Start-Sleep -Seconds 1
        $mark = Mark
        C 'sc spawn bog_slime 0 1' | Out-Null
        Start-Sleep -Seconds 2
        $pos = Get-MobPos 'soulscraft:bog_slime'
        C "damage @e[type=soulscraft:bog_slime,limit=1,sort=nearest,x=$($pos[0]),y=$($pos[1]),z=$($pos[2])] 1000 minecraft:player_attack by sc_a" | Out-Null
        Start-Sleep -Seconds 3
        $adopts = @(Get-Rows $mark 'adopt' | Where-Object { $_.mob -eq 'bog_slime' })
        $two = @($adopts | Where-Object { $_.size -eq '2' }).Count
        $count = (C "execute if entity @e[type=soulscraft:bog_slime,x=$cx,y=$cy,z=$cz,distance=..30]") -join ' '
        Note 'bog slime split' (@($adopts | ForEach-Object { "adopt size=$($_.size) fresh=$($_.fresh)" }) + "size-2 adopts: $two; server says: $count")
        Take-Shot $script:Bot 'bog_slime_split'
        if ([math]::Abs($height - $want) -gt 0.3) { return V 'FAIL' "the jump's apex is $([math]::Round($height, 2)) blocks, vanilla physics at jump_velocity $jv gives $([math]::Round($want, 2))" }
        $total = if ($count -match 'count: (\d+)') { [int]$Matches[1] } else { -1 }
        if ($two -in 2..3 -or $total -in 2..3) { return V 'PASS' "apex $([math]::Round($height, 2)) (vanilla $([math]::Round($want, 2)) at $jv); split into $two size-2 slimes (server count $total); the smoke shadow is for the eye in bog_slime_midjump_*" }
        V 'FAIL' "apex $([math]::Round($height, 2)); after the kill $two size-2 adopts, server count $total (want 2-3)"
    }

    Run-Scene 'lurker_grab' {
        # The arena's pool (tools.rs: x -493..-489, z 507..511, water to y 100). The bot swims in it
        # (survival, resistance). Frozen and stepped from the summon on, so the grab (grab_max_ticks
        # 40) is never half over before the blows: 20 ticks to settle and adopt, then `sc force
        # ambush` if no grab came by itself, stepped 2 ticks at a time until `mode=grab`. The three
        # blows are the console's `damage <lurker> 1 minecraft:player_attack by sc_a` (the bot's own
        # `attack` is refused under water as out of reach; the mod counts a player_attack hit while
        # the grabbed player is the nearest, server.rs note_grab_blow), 11 ticks apart (past the
        # lurker's hurt cooldown, where a blow takes no health and does not count). Freed: a
        # `mode=backoff` line after the grab and before grab_max_ticks, and the lurker 2+ blocks off.
        Set-Survivor
        $sel = '@e[type=soulscraft:lurker,limit=1,sort=nearest,x=-491.5,y=100,z=509]'
        $mark = Mark
        C 'tick freeze' | Out-Null
        C 'summon soulscraft:lurker -491.5 99.6 510.5' | Out-Null
        C 'tp sc_a -491.5 100.2 507.5 0 20' | Out-Null
        Start-Sleep -Seconds 1
        Step-World 20
        $grabRow = { @(Get-Rows $mark 'mode' | Where-Object { $_.mob -eq 'lurker' -and $_.mode -eq 'grab' }) | Select-Object -First 1 }
        $g = & $grabRow
        if (-not $g) { C 'sc force ambush' | Out-Null }
        for ($i = 0; $i -lt 30 -and -not $g; $i++) {
            C 'tick step 2' | Out-Null
            Start-Sleep -Milliseconds 300
            $g = & $grabRow
        }
        if (-not $g) {
            C 'tick unfreeze' | Out-Null
            Note 'lurker (no grab seen)' (Get-LogSince $mark 'soulscraft: (phase|strike|adopt|mode) ')
            return V 'FAIL' 'no grab in 80 stepped ticks (natural or forced ambush)'
        }
        Start-Sleep -Milliseconds 800
        Take-Shot $script:Bot 'lurker_grabbed'
        $l = Get-MobPos 'soulscraft:lurker' @(-491.5, 100, 509)
        $near = Dist2 (Get-BotPos) $l
        $blows = @()
        for ($i = 1; $i -le 3; $i++) {
            $h0 = ((C "data get entity $sel Health") -join ' ') -replace '.*: ', ''
            C "damage $sel 1 minecraft:player_attack by sc_a" | Out-Null
            $h1 = ((C "data get entity $sel Health") -join ' ') -replace '.*: ', ''
            $blows += "blow ${i}: lurker health $h0 -> $h1"
            Step-World 11
        }
        $free = @(Get-Rows $mark 'mode' | Where-Object { $_.mob -eq 'lurker' -and $_.mode -eq 'backoff' -and [int64]$_.tick -gt [int64]$g.tick }) | Select-Object -First 1
        # Its backoff lasts 30 ticks, then it engages and may grab again (the first re-run's 2 s
        # wait caught that second grab): the distance is read 26 ticks on, still frozen, in the backoff.
        # Held means within 1.5 blocks of its mouth (selftest lurker_grab_holds); freed, it swims off.
        Step-World 26
        $l2 = Get-MobPos 'soulscraft:lurker' @(-491.5, 100, 509)
        $far = if ($l2) { Dist2 (Get-BotPos) $l2 } else { 99 }
        Take-Shot $script:Bot 'lurker_freed'
        C 'tick unfreeze' | Out-Null
        $after = if ($free) { [int64]$free.tick - [int64]$g.tick } else { $null }
        Note 'lurker grab' (@(Get-LogSince $mark 'soulscraft: (phase|strike|mode|death) mob=lurker') + "bot $([math]::Round($near, 2)) blocks from the lurker at the grab" + $blows + "backoff $after ticks into the grab; distance 26 ticks after the third blow's step $([math]::Round($far, 2))")
        if (-not $free) { return V 'FAIL' "grabbed ($([math]::Round($near, 2)) blocks off), but no backoff after 3 player_attack blows" }
        if ($after -ge 40) { return V 'FAIL' "backed off only $after ticks into the grab: grab_max_ticks ran out, the blows did not free it" }
        if ($far -le 1.5 -or $far -lt $near + 0.5) { return V 'FAIL' "freed at +$after ticks, but the lurker is still $([math]::Round($far, 2)) blocks off in its backoff" }
        V 'PASS' "grabbed ($([math]::Round($near, 2)) blocks off), three blows freed it $after ticks into the grab (< 40); $([math]::Round($far, 2)) blocks apart in its backoff"
    }

    Run-Scene 'altar_menu' {
        # The menu is a mod screen drawn by the client, so the bot cannot read its text: the shot
        # carries the proof (four events with their horns, no «До уровня» without sky), and the
        # script only checks that a screen opened. Names expected, from data/mobs.json and the lang file:
        $lang = Get-Content -Raw -Encoding utf8 (Join-Path $Repo 'mod\content\assets\soulscraft\lang\ru_ru.json') | ConvertFrom-Json
        $want = @($MobData.events | ForEach-Object { "$($_.name) / $($lang.("item." + ($_.item -replace ':', '.')))" })
        C 'clear sc_a' | Out-Null
        C 'sc altar 4' | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $script:Bot 'select 9' | Out-Null
        C "tp sc_a $($ax + 0.5) $ay $($az - 2.5) 0 40" | Out-Null
        Start-Sleep -Seconds 1
        Invoke-Bot $script:Bot "interact $ax $ay $az" | Out-Null
        Start-Sleep -Seconds 2
        $screen = (Invoke-Bot $script:Bot 'screen') -join "`n"
        $st = Get-BotState
        Save-Shot $script:Bot 'altar_menu_four_events'
        Note 'altar menu' (@('expected in the menu (event / horn):') + $want + 'bot screen reply:' + $screen)
        $names = @($MobData.events | ForEach-Object { $_.name }) + @($want | ForEach-Object { ($_ -split ' / ')[1] })
        $found = @($names | Where-Object { $screen -like "*$_*" }).Count
        Invoke-Bot $script:Bot 'key esc tap' | Out-Null
        if (-not $st.screen -or "$($st.screen)" -match '^(none|null)$') { return V 'FAIL' 'no screen opened on the core' }
        if ($found -eq $names.Count) { return V 'PASS' 'all four event names and horn names are in the screen reply' }
        V 'SEE-SHOT' "a screen opened ($found of $($names.Count) names readable by the bot); check altar_menu_four_events.jpg for four events with horns and no «До уровня»"
    }

    Run-Scene 'event_zone' {
        # Tier 2 (grave_night, 6 waves). The zone is 3x3 chunks round the altar's chunk (x -528..-481,
        # z 480..527); its outline shows for outline_ticks = 100 (5 s). The camera sc_b stands inside
        # it 6 blocks up, about 10 blocks from the edge, looking down at an angle: at the north-west
        # corner post (shot a, ~0.5 s after the horn), then at the north rail's middle (shot b, ~2 s).
        # The boss bar: the server is asked who sees it (`bossbar get <id> players`: the bot must be
        # on it; the event sets the players once a second, event_ui.rs show_bar) and the bot's own
        # view is shot 1.5 s in. Then the wave, and the wave 2 skeletons in the log (wave 1 killed).
        $cam = Get-Cam
        Set-Survivor
        C 'sc altar 2 banners' | Out-Null
        Start-Sleep -Seconds 2
        C "tp sc_b -518.5 107 490.5" | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $cam 'look-at -528 101 480' | Out-Null
        Start-Sleep -Seconds 1
        $mark = Mark
        Start-Event 'soulscraft:grave_horn'
        Start-Sleep -Milliseconds 300
        Take-Shot $cam 'event_zone_outline_a'
        C "tp sc_b -500.5 107 490.5" | Out-Null
        Invoke-Bot $cam 'look-at -500 101 480' | Out-Null
        Start-Sleep -Milliseconds 700
        $bars = @(C 'bossbar list')
        Take-Shot $script:Bot 'event_bossbar'
        Take-Shot $cam 'event_zone_outline_b'
        $barId = $null
        foreach ($n in 1..100) {
            $r = (C "bossbar get soulscraft:altar_$n visible") -join ' '
            if ($r -match 'is currently (visible|hidden)') { $barId = "soulscraft:altar_$n" }
        }
        $barPlayers = if ($barId) { (C "bossbar get $barId players") -join ' ' } else { 'no soulscraft:altar_N bar found' }
        $started = @(Get-LogSince $mark 'soulscraft: event start')
        Note 'event start' ($started + "bossbar list: $($bars -join ' ')" + "bossbar $barId players: $barPlayers" + "bot messages: $((Invoke-Bot $script:Bot 'messages') -join ' ')")
        Start-Sleep -Seconds 7
        Invoke-Bot $cam "look-at $ax $($ay + 1) $az" | Out-Null
        C "tp sc_b $($ax - 14) $($ay + 10) $($az - 14) 0 0" | Out-Null
        Invoke-Bot $cam "look-at $ax $($ay + 1) $az" | Out-Null
        Start-Sleep -Seconds 1
        Take-Shot $cam 'event_wave1'
        $spawn1 = @(Get-LogSince $mark 'soulscraft: event spawn wave=1 ').Count
        $barText = ($bars -join ' ')
        $barOk = $barText -match 'Волна 1 из 6' -and $barText -match 'Ночь мертвецов'
        # Wave 2: kill wave 1, wait out the rest, watch the skeletons 25 s.
        $reached = Skip-ToWave 2 $mark 120
        $skel = @()
        if ($reached) {
            Start-Sleep -Seconds 25
            $rows = @(Get-Rows $mark 'vadopt|vphase|vstrike|vmode|event')
            $skelIds = @($rows | Where-Object { $_.kind -eq 'vadopt' -and $_.mob -eq 'minecraft:skeleton' } | ForEach-Object { $_.id })
            $skel = @($rows | Where-Object { (($_.kind -in ('vphase', 'vstrike', 'vmode')) -and $_.mob -eq 'minecraft:skeleton') -or ($_.kind -eq 'event' -and $_.mob -in $skelIds) })
            Take-Shot $cam 'event_wave2_skeletons'
        }
        Note 'event wave 2 skeleton lines' (@($skel | ForEach-Object { ($_.PSObject.Properties | ForEach-Object { "$($_.Name)=$($_.Value)" }) -join ' ' }) + "skeleton vadopt ids: $($skelIds -join ',')")
        if ($started.Count -eq 0) { return V 'FAIL' 'the horn did not start the event (no `event start` line)' }
        if (-not $barOk) { return V 'FAIL' "boss bar list lacks «Волна 1 из 6 · Ночь мертвецов»: $barText" }
        if ($barPlayers -notmatch 'sc_a') { return V 'FAIL' "the boss bar $barId is not shown to the bot, a participant: $barPlayers" }
        if ($spawn1 -eq 0) { return V 'FAIL' 'no `event spawn wave=1` line in 11 s' }
        if (-not $reached) { return V 'FAIL' 'wave 2 never spawned in 120 s after killing wave 1' }
        if ($skel.Count -eq 0) { return V 'FAIL' 'wave 2 came, but no skeleton vphase/vstrike/vmode line (the skeletons neither shot nor moved by their group)' }
        V 'SEE-SHOT' "event started, bar «Волна 1 из 6 · Ночь мертвецов» shown to sc_a (bossbar get players), wave 1 spawned, $($skel.Count) skeleton behaviour line(s) in wave 2; the zone outline (rails, posts) is in event_zone_outline_a/b.jpg, the bar in event_bossbar.jpg"
    }

    Run-Scene 'event_bearer' {
        # The banner-bearer belongs to test_zombies (tier 1, rotten horn, wave 5 `banner_zombie`,
        # bearer_scale 1.3): wave mobs are killed until wave 5 spawns (4 rests of 30-45 s, ~3-4 min).
        # Skipped when -Only names other scenes.
        $cam = Get-Cam
        Set-Survivor
        C 'sc altar 1 banners' | Out-Null
        Start-Sleep -Seconds 2
        $mark = Mark
        Start-Event 'soulscraft:rotten_horn'
        $reached = Skip-ToWave 5 $mark 400
        if (-not $reached) { return V 'FAIL' 'wave 5 never spawned in 400 s' }
        Start-Sleep -Seconds 4
        C "tp sc_b $($ax - 10) $($ay + 5) $($az - 12) 0 0" | Out-Null
        Invoke-Bot $cam "look-at $ax $($ay + 1) $az" | Out-Null
        Start-Sleep -Seconds 1
        Take-Shot $cam 'event_bearer_wave5'
        # The close shot: the world frozen, the bearer told by its health (health_mult x3 over the
        # other zombies'; `/attribute ... minecraft:scale get` reads the engine's own 1.0, while the
        # event grows it in the mod's attribute map and model scale, event.rs set_scale). The bearer
        # and one ordinary zombie are tagged and the rest of the wave killed (teleporting event mobs
        # does not hold: the event drives them back), stepped 25 ticks for the bodies to go, then the
        # camera 5 blocks north of the pair at their height.
        C 'tick freeze' | Out-Null
        $ents = Invoke-Bot $cam 'entities 60 zombie' | Out-String
        Set-Content (Join-Path $Out 'event_bearer_entities.json') $ents
        $zs = @()
        try { $zs = @(($ents | ConvertFrom-Json).entities | Where-Object { $_.type -eq 'minecraft:zombie' }) } catch { }
        $hs = @($zs | ForEach-Object { [double]$_.health } | Sort-Object)
        $median = if ($hs.Count) { $hs[[int]($hs.Count / 2)] } else { 0 }
        $bearer = $zs | Where-Object { [double]$_.health -ge 2 * $median } | Select-Object -First 1
        $plain = $zs | Where-Object { [double]$_.health -eq $median } | Select-Object -First 1
        $pair = @()
        if ($bearer -and $plain) {
            foreach ($z in $bearer, $plain) {
                C "tag @e[type=minecraft:zombie,limit=1,sort=nearest,x=$($z.pos[0]),y=$($z.pos[1]),z=$($z.pos[2])] add sc.bearer_pair" | Out-Null
            }
            C 'kill @e[tag=soulscraft.wave,tag=!sc.bearer_pair]' | Out-Null
            Step-World 25
            foreach ($i in 0, 1) {
                $r = (C "data get entity @e[tag=sc.bearer_pair,limit=1,sort=$(if ($i) { 'furthest' } else { 'nearest' }),x=$ax,y=$ay,z=$az] Pos") -join ' '
                if ($r -match '\[(-?[\d.]+)d, (-?[\d.]+)d, (-?[\d.]+)d\]') { $pair += , @([double]$Matches[1], [double]$Matches[2], [double]$Matches[3]) }
            }
            if ($pair.Count -eq 2) {
                $mid = @((($pair[0][0] + $pair[1][0]) / 2), [math]::Min($pair[0][1], $pair[1][1]), (($pair[0][2] + $pair[1][2]) / 2))
                C "tp sc_b $($mid[0]) $($mid[1] + 1.2) $($mid[2] - 5)" | Out-Null
                Start-Sleep -Milliseconds 800
                Invoke-Bot $cam "look-at $($mid[0]) $($mid[1] + 1.1) $($mid[2])" | Out-Null
                Start-Sleep -Seconds 1
                Take-Shot $cam 'event_bearer_beside_zombie'
            }
            C 'tag @e[tag=sc.bearer_pair] remove sc.bearer_pair' | Out-Null
        }
        C 'tick unfreeze' | Out-Null
        $who = if ($bearer) { "health $($bearer.health) at $(($bearer.pos | ForEach-Object { [math]::Round($_, 1) }) -join ' ')" } else { 'none told by health' }
        Note 'banner-bearer' @("zombie healths: $($hs -join ' ') (median $median)", "bearer: $who", "pair after the others were killed: $(($pair | ForEach-Object { ($_ | ForEach-Object { [math]::Round($_, 1) }) -join ' ' }) -join ' and ')", 'entities dump in event_bearer_entities.json')
        if ($bearer -and $plain) { return V 'SEE-SHOT' "the bearer (health $([math]::Round([double]$bearer.health, 1)), an ordinary one $([math]::Round([double]$plain.health, 1))) and an ordinary zombie alone in event_bearer_beside_zombie.jpg: check it is 1.3x" }
        V 'FAIL' "wave 5 spawned, but no zombie with the bearer's x3 health among $($zs.Count) (healths $($hs -join ' '))"
    }

    Run-Scene 'record' {
        # The LetsCam client mod records sc_a's view of the tier-2 fight (`/letscam start|stop`,
        # as in letscam/tools/bench-test.ps1), then `/letscam render auto sheet` makes the mp4 and its
        # contact sheet in $LcOut. record_mic = false is in the config set above (never a microphone).
        # The client read that config at launch, so sc_a is restarted here first: stopped, and its
        # control socket gone (the game quits a moment after `quit`), before the new launch.
        Stop-Bot $script:Bot
        $deadline = (Get-Date).AddSeconds(60)
        while ((Get-Date) -lt $deadline) {
            & $script:Ctl --socket $script:Bot.Socket --timeout 3 state 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) { break }
            Start-Sleep -Seconds 1
        }
        Start-Sleep -Seconds 3
        Remove-Item -LiteralPath $script:Bot.Socket -Force -ErrorAction SilentlyContinue
        $script:Bot = Start-Bot sc_a
        Set-Survivor
        $since = Get-Date
        $botLog = [IO.Path]::ChangeExtension($script:Bot.Socket, 'log')
        $waitMsg = {
            param([string[]]$Patterns, [int]$Sec)
            $deadline = (Get-Date).AddSeconds($Sec)
            while ((Get-Date) -lt $deadline) {
                $t = (Invoke-Bot $script:Bot 'messages' 10) -join "`n"
                if (Test-Path $botLog) { $t += "`n" + ((Get-Content -Tail 200 $botLog) -join "`n") }
                foreach ($p in $Patterns) { if ($t -match $p) { return $Matches[0] } }
                Start-Sleep -Seconds 2
            }
            throw "none of [$($Patterns -join ', ')] after $Sec s"
        }
        C 'sc altar 2 banners' | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $script:Bot 'chat /letscam start' | Out-Null
        $said = & $waitMsg @('Запись началась', 'не начал') 20
        if ($said -match 'не начал') { return V 'FAIL' "LetsCam would not start: $said" }
        Start-Sleep -Seconds 3
        Start-Event 'soulscraft:grave_horn'
        for ($i = 0; $i -lt 12; $i++) {
            Invoke-Bot $script:Bot "look $(($i * 40) % 360 - 180) 15" | Out-Null
            Start-Sleep -Seconds 4
        }
        Invoke-Bot $script:Bot 'chat /letscam stop' | Out-Null
        $said = & $waitMsg @('Запись сохранена[^\n]*', 'остановлена с ошибкой[^\n]*') 40
        C 'sc event stop' | Out-Null
        if ($said -match 'ошибк') { return V 'FAIL' "recording stopped with an error: $said" }
        Invoke-Bot $script:Bot 'chat /letscam render auto sheet' | Out-Null
        $deadline = (Get-Date).AddSeconds(900); $done = $null
        while ((Get-Date) -lt $deadline -and -not $done) {
            $lines = if (Test-Path $botLog) { @(Get-Content $botLog | Where-Object { $_ -match 'letscam: ' }) } else { @() }
            $fail = $lines | Where-Object { $_ -match 'Рендер остановлен|не удался|не открылся|не начался' } | Select-Object -First 1
            if ($fail) { return V 'FAIL' "render: $fail" }
            if (@($lines | Where-Object { $_ -match 'Ролик готов' }).Count -ge 1) { $done = $true }
            Start-Sleep -Seconds 3
        }
        if (-not $done) { return V 'FAIL' 'the render was not done in 900 s' }
        $dir = Get-ChildItem $LcOut -Directory | Where-Object { $_.LastWriteTime -ge $since } | Sort-Object Name | Select-Object -Last 1
        if (-not $dir) { return V 'FAIL' "no recording folder in $LcOut" }
        $video = Get-ChildItem $dir.FullName -File | Where-Object { $_.Extension -eq '.mp4' } | Sort-Object Length -Descending | Select-Object -First 1
        if (-not $video) { return V 'FAIL' "no mp4 in $($dir.FullName)" }
        Copy-Item $video.FullName (Join-Path $Out 'tier2_event_fight.mp4') -Force
        $sheets = @(Get-ChildItem $dir.FullName -File -Filter '*_sheet_*.png')
        foreach ($s in $sheets) { Copy-Item $s.FullName (Join-Path $Out "tier2_event_$($s.Name)") -Force }
        Note 'record' @("source folder: $($dir.FullName)", "mp4: $($video.Name) $([math]::Round($video.Length / 1MB, 1)) MB", "contact sheets: $($sheets.Count)")
        if ($video.Length -lt 100KB) { return V 'FAIL' "the mp4 is only $($video.Length) bytes" }
        V 'PASS' "tier2_event_fight.mp4 ($([math]::Round($video.Length / 1MB, 1)) MB) and $($sheets.Count) contact sheet(s) copied"
    }
} finally {
    foreach ($c in 'tick unfreeze', 'sc slowmo off', 'sc event stop', 'sc clear', 'sc altar 0', 'effect clear sc_a', 'gamemode creative sc_a',
        'gamerule advance_time true', 'sc debug off') {
        try { C $c | Out-Null } catch { Write-Warning "cleanup $c`: $_" }
    }
    if ($script:Cam) { try { C 'gamemode creative sc_b' | Out-Null; Stop-Bot $script:Cam } catch { Write-Warning "camera: $_" } }
    try { Stop-Bot $script:Bot } catch { Write-Warning "bot: $_" }
    if (Scene 'record') {
        if ($null -ne $LcBackup) { Set-Content -Encoding utf8 -NoNewline $LcConfig $LcBackup }
        else { Remove-Item -LiteralPath $LcConfig -Force -ErrorAction SilentlyContinue }
    }
}

Add-Content $Text "`n######## summary"
$table = $script:Results | Format-Table Scene, Status, Reason -AutoSize -Wrap | Out-String -Width 200
Add-Content $Text $table
Write-Host $table
Write-Host "Evidence in $Out"
if (@($script:Results | Where-Object Status -eq 'FAIL').Count -gt 0) { exit 1 }

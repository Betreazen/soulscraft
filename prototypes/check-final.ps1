# SoulsCraft 0.7.0's new behaviour in game (docs/superpowers/plans/2026-11-16-final-version.md,
# tasks 1-3), bots sc_a (the target) and sc_b (the camera, a spectator with the GUI hidden) on the
# joint server. Scenes, each selectable with -Only:
#   revenant_hold   the revenant steps to its target during `hold`; the shield bash knocks back
#   stalker_flank   watched, it circles and waits; looked away from, it strikes before flank_max_ticks
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
        $bp = @($cx, $cy, $cz - 3)
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

    Run-Scene 'stalker_flank' {
        # Trial A: the bot keeps looking at the stalker for 4 s (80 ticks, under flank_max_ticks 100):
        # no strike. Trial B: a fresh stalker, the bot looks away (`look`, re-aimed every 0.4 s so the
        # stalker never gets into the view): it strikes before flank_max_ticks.
        Set-Survivor
        $bp = @($cx, $cy, $cz - 3)
        $max = [int]$MobData.mobs.Where({ $_.id -eq 'stalker' })[0].params.flank_max_ticks
        if ($max -le 0) { $max = 100 }
        # A
        C 'sc spawn stalker 0 1' | Out-Null
        Start-Sleep -Seconds 1
        $markA = Mark
        $bearings = @(); $t0 = Get-Date
        while (((Get-Date) - $t0).TotalSeconds -lt 4) {
            $m = Get-MobPos 'soulscraft:stalker'
            if ($m) {
                Invoke-Bot $script:Bot "look-at $($m[0]) $($m[1] + 1) $($m[2])" | Out-Null
                $bearings += [math]::Round((Bearing $bp $m), 0)
            }
        }
        Take-Shot $script:Bot 'stalker_watched'
        $rowsA = @(Get-Rows $markA 'mode|strike|phase')
        $strikesA = @($rowsA | Where-Object kind -eq 'strike').Count
        $span = if ($bearings.Count) { ($bearings | Measure-Object -Maximum -Minimum | ForEach-Object { $_.Maximum - $_.Minimum }) } else { 0 }
        Note 'stalker watched (4 s)' (@($rowsA | ForEach-Object { ($_.PSObject.Properties | ForEach-Object { "$($_.Name)=$($_.Value)" }) -join ' ' }) + "bearings from the bot: $($bearings -join ' ') (circled $span degrees); strikes $strikesA")
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
        if ($strikesA -gt 0) { return V 'FAIL' "struck $strikesA time(s) within 4 s while watched" }
        if (-not $struck) { return V 'FAIL' 'never struck in 9 s with the bot looking away' }
        if ($ticks -ge $max) { return V 'FAIL' "struck $ticks ticks after adopt, not before flank_max_ticks $max (a timeout, not an unseen strike)" }
        V 'PASS' "watched: no strike in 4 s (circled $span degrees); looked away from: struck $ticks ticks after adopt (< $max), $offAtStrike degrees off the view"
    }

    Run-Scene 'marksman_cover' {
        # Wall x -496..-494, y 101..103, z 502 (3x3), 5.4 blocks from the marksman at z 500.5; the
        # bot is its target 10 blocks north. After the shot the marksman reloads: behind the wall means
        # its line to the bot crosses the wall's plane inside the wall and it stands past it.
        $cam = Get-Cam
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
        $bp = @($cx, $cy, $cz - 10)
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
        # overhead_wave (windup 16) with the bot 2 blocks in front of the brute, the impact point.
        # Standing: health must drop. Jumping: `key space tap` at three delays after the console's
        # answer, one of which should put the bot in the air (feet > 0.5 over the ground) at the hit
        # tick. Timing by wall clock: a jump that still loses health may only be a mistimed one (see
        # the per-attempt lines and the strike log).
        $trial = {
            param([double]$JumpAfter)
            C 'sc clear' | Out-Null
            C 'gamemode survival sc_a' | Out-Null
            C 'effect clear sc_a' | Out-Null
            C 'effect give sc_a minecraft:instant_health 1 10 true' | Out-Null
            C "tp sc_a $cx $cy $($cz - 3) 0 0" | Out-Null
            C 'sc spawn brute 0 1' | Out-Null
            Start-Sleep -Seconds 2
            $b = Get-MobPos 'soulscraft:brute'
            C "tp sc_a $($b[0]) $($b[1]) $($b[2] - 2.0) 0 0" | Out-Null
            Start-Sleep -Seconds 2
            $h0 = Get-Health
            $mark = Mark
            C 'sc force overhead_wave' | Out-Null
            if ($JumpAfter -ge 0) { Start-Sleep -Milliseconds ([int]($JumpAfter * 1000)); Invoke-Bot $script:Bot 'key space tap' | Out-Null }
            Start-Sleep -Milliseconds 1800
            $h1 = Get-Health
            $lines = @(Get-LogSince $mark 'soulscraft: (phase|strike) mob=brute')
            [pscustomobject]@{ lost = [math]::Round($h0 - $h1, 2); lines = $lines }
        }
        $stand = & $trial (-1)
        Note 'brute standing' (@($stand.lines) + "health lost $($stand.lost)")
        Take-Shot $script:Bot 'brute_wave_standing_after'
        $jumps = @()
        foreach ($d in 0.05, 0.2, 0.35) {
            $j = & $trial $d
            $jumps += [pscustomobject]@{ delay = $d; lost = $j.lost }
            Note "brute jumping (space $d s after the console answer)" (@($j.lines) + "health lost $($j.lost)")
        }
        $dodged = @($jumps | Where-Object { $_.lost -le 0 })
        $summary = ($jumps | ForEach-Object { "$($_.delay)s:-$($_.lost)" }) -join ' '
        if ($stand.lost -le 0) { return V 'FAIL' "the standing bot lost no health (wave did not hit; jumps $summary)" }
        if ($dodged.Count -eq 0) { return V 'FAIL' "standing lost $($stand.lost); no jump attempt dodged it ($summary): the wave hits jumpers or the timing missed, see console.txt" }
        V 'PASS' "standing lost $($stand.lost); jumping $summary ($($dodged.Count) of 3 attempts took nothing)"
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
        # A: frozen, `sc force high_jump`, the slime stepped through its jump with the camera at the
        # side. Its height is measured; the smoke shadow at the landing spot is in the shots only.
        # B: a size-4 slime killed by the bot, the size-2 slimes counted (adopt lines, and the
        # server's count of bog slimes).
        $cam = Get-Cam
        Set-Survivor
        C "tp sc_b $($cx + 6) $($cy + 2.5) $($cz - 0.5) -90 10" | Out-Null
        C 'sc spawn bog_slime 0 1' | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $cam "look-at $cx $($cy + 1) $($cz - 0.5)" | Out-Null
        $ground = (Get-MobPos 'soulscraft:bog_slime')[1]
        C 'tick freeze' | Out-Null
        C 'sc force high_jump' | Out-Null
        $a = Get-Attack 'bog_slime' 'high_jump'
        $at = -1; $top = $ground; $lines = @()
        foreach ($stop in ($a.windup + 3), ($a.windup + 9), ($a.windup + 15), ($a.windup + 21)) {
            Step-World ($stop - $at); $at = $stop
            $p = Get-MobPos 'soulscraft:bog_slime'
            if ($p) { $top = [math]::Max($top, $p[1]); $lines += "t=$at slime at $(($p | ForEach-Object { [math]::Round($_, 2) }) -join ',')" }
            Take-Shot $cam "bog_slime_midjump_t$at"
        }
        C 'tick unfreeze' | Out-Null
        $height = $top - $ground
        Note "bog slime jump (ground y $ground)" $lines
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
        if ($height -lt 0.25) { return V 'FAIL' "the slime rose only $([math]::Round($height, 2)) blocks in the jump steps" }
        $total = if ($count -match 'count: (\d+)') { [int]$Matches[1] } else { -1 }
        if ($two -in 2..3 -or $total -in 2..3) { return V 'PASS' "jump rose $([math]::Round($height, 2)); split into $two size-2 slimes (server count $total); the smoke shadow is for the eye in bog_slime_midjump_*" }
        V 'FAIL' "jump rose $([math]::Round($height, 2)); after the kill $two size-2 adopts, server count $total (want 2-3)"
    }

    Run-Scene 'lurker_grab' {
        # The arena's pool (tools.rs: x -493..-489, z 507..511, water to y 100). The bot swims in it
        # (survival, resistance). The mod prints no grab line of its own: a grab is read from a
        # `phase/strike ... attack=grab|ambush hit=true` line and the bot staying within 1.6 blocks of
        # the lurker. Then three blows (ctl attack on the lurker's id) and the bot must come loose.
        Set-Survivor
        C 'summon soulscraft:lurker -491.5 99.6 510.5' | Out-Null
        C 'tp sc_a -491.5 100.2 507.5 0 20' | Out-Null
        Start-Sleep -Seconds 2
        $mark = Mark
        $grabbed = $false; $t0 = Get-Date; $forced = $false; $near = 0
        while (((Get-Date) - $t0).TotalSeconds -lt 25 -and -not $grabbed) {
            Start-Sleep -Milliseconds 600
            $l = Get-MobPos 'soulscraft:lurker' @(-491.5, 100, 509)
            $b = Get-BotPos
            $lines = @(Get-Rows $mark 'phase|strike' | Where-Object { $_.mob -eq 'lurker' })
            $hit = @($lines | Where-Object { $_.attack -eq 'grab' -or ($_.kind -eq 'strike' -and $_.attack -eq 'ambush' -and $_.hit -eq 'true') }).Count -gt 0
            $near = if ($l) { Dist2 $b $l } else { 99 }
            $grabbed = $hit -and $near -le 1.8
            if (-not $forced -and ((Get-Date) - $t0).TotalSeconds -gt 8) { C 'sc force ambush' | Out-Null; $forced = $true }
        }
        Take-Shot $script:Bot 'lurker_grabbed'
        if (-not $grabbed) {
            Note 'lurker (no grab seen)' (Get-LogSince $mark 'soulscraft: (phase|strike|adopt|mode) ')
            return V 'FAIL' "no grab in 25 s (last distance bot-lurker $([math]::Round($near, 2)))"
        }
        $ents = Invoke-Bot $script:Bot 'entities 8' | Out-String | ConvertFrom-Json
        $id = ($ents.entities | Where-Object { $_.type -like '*lurker' } | Select-Object -First 1).id
        $replies = @()
        if ($id) {
            for ($i = 1; $i -le 3; $i++) {
                Invoke-Bot $script:Bot "look-at-entity $id" | Out-Null
                $replies += "blow ${i}: " + ((Invoke-Bot $script:Bot "attack $id") -join ' ')
                Start-Sleep -Milliseconds 800
            }
        }
        Start-Sleep -Seconds 2
        $l2 = Get-MobPos 'soulscraft:lurker' @(-491.5, 100, 509)
        $far = if ($l2) { Dist2 (Get-BotPos) $l2 } else { 99 }
        Take-Shot $script:Bot 'lurker_freed'
        Note 'lurker grab' (@(Get-LogSince $mark 'soulscraft: (phase|strike|mode|death) mob=lurker') + $replies + "distance after the blows $([math]::Round($far, 2)); lurker id $id")
        if (-not $id) { return V 'FAIL' 'grabbed, but the bot saw no lurker entity to hit' }
        if ($far -ge 2.0 -or -not $l2) { return V 'PASS' "grabbed (within 1.6), freed by 3 blows: $([math]::Round($far, 2)) blocks apart after 2 s" }
        V 'FAIL' "three blows did not free the bot: still $([math]::Round($far, 2)) blocks from the lurker"
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
        # Tier 2 (grave_night, 6 waves): the camera sc_b hangs 28 blocks up and 34 north of the altar
        # before the horn; shots at about 0.5, 1.5 and 2.5 s after the click (the outline shows for
        # outline_ticks = 100, 5 s) and the boss bar from the bot's own view; then the wave, and the
        # wave 2 skeletons in the log (wave 1 is killed to reach it).
        $cam = Get-Cam
        Set-Survivor
        C 'sc altar 2 banners' | Out-Null
        Start-Sleep -Seconds 2
        C "tp sc_b $($ax + 0.5) $($ay + 28) $($az - 34) 0 40" | Out-Null
        Start-Sleep -Seconds 3
        $mark = Mark
        Start-Event 'soulscraft:grave_horn'
        Take-Shot $cam 'event_zone_outline_a'
        $bars = @(C 'bossbar list')
        Take-Shot $script:Bot 'event_bossbar'
        Take-Shot $cam 'event_zone_outline_b'
        $started = @(Get-LogSince $mark 'soulscraft: event start')
        Note 'event start' ($started + "bossbar list: $($bars -join ' ')" + "bot messages: $((Invoke-Bot $script:Bot 'messages') -join ' ')")
        Start-Sleep -Seconds 8
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
        if ($spawn1 -eq 0) { return V 'FAIL' 'no `event spawn wave=1` line in 11 s' }
        if (-not $reached) { return V 'FAIL' 'wave 2 never spawned in 120 s after killing wave 1' }
        if ($skel.Count -eq 0) { return V 'FAIL' 'wave 2 came, but no skeleton vphase/vstrike/vmode line (the skeletons neither shot nor moved by their group)' }
        V 'SEE-SHOT' "event started, bar «Волна 1 из 6 · Ночь мертвецов» in bossbar list, wave 1 spawned, $($skel.Count) skeleton behaviour line(s) in wave 2; the zone outline within 5 s is only in event_zone_outline_a/b.jpg"
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
        $ents = Invoke-Bot $cam 'entities 60 zombie' | Out-String
        Set-Content (Join-Path $Out 'event_bearer_entities.json') $ents
        $sizes = @()
        try {
            $z = ($ents | ConvertFrom-Json).entities
            foreach ($e in $z) { foreach ($n in 'scale', 'height') { if ($e.PSObject.Properties.Name -contains $n -and $e.$n -is [ValueType]) { $sizes += [double]$e.$n } } }
        } catch { }
        Note 'banner-bearer' @("entities dump in event_bearer_entities.json; sizes seen: $($sizes -join ' ')")
        if ($sizes.Count -ge 2) {
            $sorted = $sizes | Sort-Object
            $ratio = $sorted[-1] / [math]::Max($sorted[[int]($sorted.Count / 2)], 0.01)
            if ($ratio -ge 1.25) { return V 'PASS' "the largest zombie is $([math]::Round($ratio, 2)) times the median" }
        }
        V 'SEE-SHOT' 'wave 5 spawned; the bot cannot read minecraft:scale, check event_bearer_wave5.jpg for the one big zombie (1.3x)'
    }

    Run-Scene 'record' {
        # The LetsCam client mod records sc_a's view of the tier-2 fight (`/letscam start|stop`,
        # as in letscam/tools/bench-test.ps1), then `/letscam render auto sheet` makes the mp4 and its
        # contact sheet in $LcOut. record_mic = false is in the config set above (never a microphone).
        # The client read that config at launch, so sc_a is restarted here first.
        Stop-Bot $script:Bot
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

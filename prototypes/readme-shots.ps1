# Clean shots for the repository's README (docs/images/): the HUD hidden (F1), the tutorial toast
# off, the debug labels off. Scenes: the seven mobs of the first slice in a row, the brute's
# overhead swing frozen in its windup, the altar's event at dusk with the drain beams, the
# dioramas at night and one close up, the hunter's book on the brute's page.
# JPEG 1280 wide in docs/images/; the PNGs stay in the scratchpad.
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft session"   (and stop, close after)
#   -Only lineup,brute,altar,dioramas,book   retake some scenes only
param([string[]]$Only)
$ErrorActionPreference = 'Stop'
function Scene([string]$Name) { -not $Only -or $Only -contains $Name }
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path (Split-Path $PSScriptRoot) 'docs\images'
$Raw = Join-Path $script:Scratch 'readme'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
$Mobs = 'revenant', 'stalker', 'marksman', 'brute', 'shrike', 'lurker', 'bog_slime'
# The arena (tools.rs ARENA): a flat lit floor at y 100 round (-500, 500); `/sc altar` builds
# 6 blocks south of its centre.
$ax = -500; $ay = 101; $az = 500

function Save-Shot($Bot, [string]$Name) {
    $png = Join-Path $Raw "$Name.png"
    Invoke-Bot $Bot "screenshot $png" | Out-Null
    $jpg = Join-Path $Out "$Name.jpg"
    $ps = "Add-Type -AssemblyName System.Drawing; `$i=[Drawing.Image]::FromFile('$png'); `$w=[Math]::Min(1280, `$i.Width); `$h=[int](`$i.Height*`$w/`$i.Width); " +
        "`$b=New-Object Drawing.Bitmap `$i, `$w, `$h; `$c=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders()|?{`$_.MimeType -eq 'image/jpeg'}; " +
        "`$p=New-Object Drawing.Imaging.EncoderParameters 1; `$p.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::Quality, [long]90); " +
        "`$b.Save('$jpg', `$c, `$p); `$b.Dispose(); `$i.Dispose()"
    powershell.exe -NoProfile -Command $ps
    Write-Host "  shot $jpg"
}
function C([string]$Line) { Invoke-Console $Line | Out-Null }
function Mark { (Get-Content $log).Count }
function Wait-Log([int]$Since, [string]$Pattern, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        if (Get-Content $log | Select-Object -Skip $Since | Select-String $Pattern | Select-Object -First 1) { return $true }
        Start-Sleep -Milliseconds 500
    }
    $false
}
function View([double]$X, [double]$Y, [double]$Z, [double]$LX, [double]$LY, [double]$LZ, [double]$Settle = 2) {
    C "tp sc_a $X $Y $Z"
    Start-Sleep -Milliseconds 400
    Invoke-Bot $bot "look-at $LX $LY $LZ" | Out-Null
    Start-Sleep -Seconds $Settle
}
function Tag([string]$Mob) {
    "Tags:[`"soulscraft.p=0.00`",`"soulscraft.fresh`"" + $(if ($Mob -eq 'bog_slime') { ',"soulscraft.size=4"' } else { '' }) + ']'
}

# No tutorial toast: the step is read from the player dir's options.txt at launch (Start-Bot keeps
# the other lines and only sets the volume).
$opts = Join-Path $script:Scratch 'bots\sc_a\options.txt'
New-Item -ItemType Directory -Force (Split-Path $opts) | Out-Null
$keep = if (Test-Path $opts) { @(Get-Content $opts | Where-Object { $_ -notmatch '^tutorialStep:' }) } else { @() }
[IO.File]::WriteAllLines($opts, [string[]](@($keep) + 'tutorialStep:none'), [Text.UTF8Encoding]::new($false))
$bot = Start-Bot sc_a
try {
    C 'tick unfreeze'
    C 'sc arena'
    C 'sc clear'
    C 'sc debug off'
    C 'gamemode creative sc_a'
    C 'clear sc_a'
    C 'weather clear'
    C 'gamerule doDaylightCycle false'
    C 'gamerule announceAdvancements false'
    C "kill @e[type=minecraft:item,x=$ax,y=$ay,z=$az,distance=..40]"
    C "fill $($ax - 14) $ay $($az - 14) $($ax + 14) $($ay + 8) $($az + 14) air"
    Start-Sleep -Seconds 1
    Invoke-Bot $bot 'key f1 tap' | Out-Null

    # 1. The seven in a row, facing the camera, in the late morning light.
    if (Scene 'lineup') {
    C 'time set 3000'
    for ($i = 0; $i -lt $Mobs.Count; $i++) {
        $x = $ax + 0.5 + 5.4 - $i * 1.8
        C "summon soulscraft:$($Mobs[$i]) $x $ay $($az + 3.5) {Rotation:[180f,0f],$(Tag $Mobs[$i]),PersistenceRequired:1b,NoAI:1b}"
    }
    # The view is ~100° wide (70° high at 16:9): 5 blocks away it holds the 11-block row.
    View ($ax + 0.5) ($ay + 1.2) ($az - 1.8) ($ax + 0.5) ($ay + 1.0) ($az + 3.5) 3
    Save-Shot $bot 'mobs_lineup'
    C 'sc clear'
    }

    # 2. The brute's overhead swing, frozen in its windup over a still golem, from the side.
    if (Scene 'brute') {
    C 'time set 3000'
    C 'tick rate 5'
    C "summon minecraft:iron_golem $($ax + 0.5) $ay $($az + 3.5) {NoAI:1b,Invulnerable:1b,PersistenceRequired:1b}"
    C "summon soulscraft:brute $($ax + 0.5) $ay $($az + 0.5) {Rotation:[0f,0f],$(Tag 'brute'),PersistenceRequired:1b}"
    # From its front left, past the golem's side.
    View ($ax - 3.5) ($ay + 1.4) ($az + 4.0) ($ax + 0.5) ($ay + 1.9) ($az + 0.5) 1
    $mark = Mark
    C 'sc force overhead_wave'
    # Windup 16 ticks (mobs.json) at 5 a second: frozen 13 ticks after it starts, the maul high.
    Wait-Log $mark 'soulscraft: phase mob=brute .*attack=overhead_wave phase=windup' 10 | Out-Null
    Start-Sleep -Milliseconds 2600
    C 'tick freeze'
    Start-Sleep -Seconds 2
    Save-Shot $bot 'brute_overhead_windup'
    C 'tick unfreeze'
    C 'tick rate 20'
    C 'sc clear'
    C "kill @e[type=minecraft:iron_golem,distance=..30,x=$ax,y=$ay,z=$az]"
    }

    # 3. The altar at sunset: level 4 with banners, the event's first wave draining it. The bot
    #    must stay a participant (a spectator is not one, and the event waits), so it stands on a
    #    barrier 9 blocks up, out of the wave's interception reach (6 blocks).
    if (Scene 'altar') {
    C 'sc altar 4 banners'
    C 'time set 12000'
    $cam = @(($ax - 5.5), ($ay + 7), ($az + 0.5))
    C "setblock $([Math]::Floor($cam[0])) $($cam[1]) $([Math]::Floor($cam[2])) minecraft:barrier"
    View $cam[0] ($cam[1] + 1) $cam[2] ($ax + 0.5) ($ay + 0.5) ($az + 6.5) 1
    $mark = Mark
    C 'sc event start'
    Wait-Log $mark 'soulscraft: event drain' 60 | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'altar_event_dusk'
    C 'sc event stop'
    C 'sc altar 0'
    C 'sc clear'
    C "setblock $([Math]::Floor($cam[0])) $($cam[1]) $([Math]::Floor($cam[2])) minecraft:air"
    C "kill @e[type=minecraft:item,x=$ax,y=$ay,z=$az,distance=..40]"
    C "fill $($ax - 14) $ay $($az - 14) $($ax + 14) $($ay + 8) $($az + 14) air"
    }

    # 4. The dioramas at night on stone pedestals, then the brute's close up.
    if (Scene 'dioramas') {
    C "tp sc_a $($ax + 0.5) $ay $($az + 0.5)"
    C 'time set 18000'
    for ($i = 0; $i -lt $Mobs.Count; $i++) {
        $x = $ax - 6 + 2 * $i
        C "setblock $x $ay $($az + 4) minecraft:polished_deepslate"
        C "setblock $x $($ay + 1) $($az + 4) soulscraft:$($Mobs[$i])_trophy[facing=north]"
    }
    View ($ax + 0.5) ($ay + 2.3) ($az + 0.6) ($ax + 0.5) ($ay + 1.3) ($az + 4.5) 3
    Save-Shot $bot 'dioramas_night'
    View ($ax + 1.7) ($ay + 2.2) ($az + 2.6) ($ax + 0.5) ($ay + 1.35) ($az + 4.5) 2
    Save-Shot $bot 'diorama_brute_night'
    C "fill $($ax - 7) $ay $($az + 4) $($ax + 7) $($ay + 1) $($az + 4) air"
    }

    # 5. The hunter's book on the brute's page (expert), the HUD back for the GUI.
    if (Scene 'book') {
    C 'time set 6000'
    Invoke-Bot $bot 'key f1 tap' | Out-Null
    C 'item replace entity sc_a hotbar.0 with soulscraft:hunter_book'
    C 'sc book sc_a reset'
    C 'sc book sc_a kill revenant 4'
    C 'sc book sc_a meet stalker'
    C 'sc book sc_a kill brute 8'
    C 'sc book sc_a kill zombie 15'
    Start-Sleep -Seconds 1
    Invoke-Bot $bot 'select 1' | Out-Null
    Invoke-Bot $bot 'use-item' | Out-Null
    Start-Sleep -Seconds 2
    # The window's layout (book_client.rs `layout`) from the GUI size it logs on its first draw.
    $w = 480; $h = 270
    $line = Select-String -Path (Join-Path $bot.Dir 'logs\latest.log') -Pattern 'book window drawn .*screen=\[([\d.]+), ([\d.]+), ([\d.]+), ([\d.]+)\]' | Select-Object -Last 1
    if ($line) { $w = [double]$line.Matches[0].Groups[3].Value; $h = [double]$line.Matches[0].Groups[4].Value }
    $pw = [Math]::Min($w - 16, 440); $ph = [Math]::Min($h - 16, 260)
    $px = ($w - $pw) / 2; $top = ($h - $ph) / 2 + 26
    Invoke-Bot $bot "click-at $([int]($px + 56)) $([int]($top + 3 * 11 + 5))" | Out-Null
    Start-Sleep -Seconds 1
    Save-Shot $bot 'book_brute'
    Invoke-Bot $bot 'key esc tap' | Out-Null
    }
} finally {
    C 'tick unfreeze'
    C 'tick rate 20'
    C 'gamerule doDaylightCycle true'
    C 'sc clear'
    Stop-Bot $bot
}
Write-Host "Shots in $Out"

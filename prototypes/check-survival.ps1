# The survival slice in game (docs/design/10-survival.md) with the bot sc_a on the joint server:
#   1. recipes: the server knows all 23 (`recipe give`), the tag ingredient included;
#   2. shards: ten revenants killed by the bot, the drops counted (Г15: about 3.5 expected);
#   3. tier events: each new horn on the pedestal starts its event on an altar of its level and
#      is refused one level lower;
#   4. the hardcore guard: the bot, a survival participant of a running event, is killed by
#      command; it must stay alive and out of spectator, outside the zone;
#   5. natural spawns: the bot in survival, at night, on open ground far from the lit arena, for
#      90 seconds; SoulsCraft mobs adopted without a P tag are counted (Г14).
# Shots and the console's answers go to prototypes/evidence/survival/.
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft session"   (and stop, close after)
param([string[]]$Only)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\survival'
$Raw = Join-Path $script:Scratch 'survival'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
$Text = Join-Path $Out 'console.txt'
# `/sc altar` from the console builds 6 blocks south of the arena's centre (tools.rs ARENA).
$ax = -500; $ay = 101; $az = 506
function Scene([string]$Name) { -not $Only -or $Only -contains $Name }
function Save-Shot($Bot, [string]$Name) {
    $png = Join-Path $Raw "$Name.png"
    Invoke-Bot $Bot "screenshot $png" | Out-Null
    $jpg = Join-Path $Out "$Name.jpg"
    $ps = "Add-Type -AssemblyName System.Drawing; `$i=[Drawing.Image]::FromFile('$png'); `$w=960; `$h=[int](`$i.Height*`$w/`$i.Width); " +
        "`$b=New-Object Drawing.Bitmap `$i, `$w, `$h; `$c=[Drawing.Imaging.ImageCodecInfo]::GetImageEncoders()|?{`$_.MimeType -eq 'image/jpeg'}; " +
        "`$p=New-Object Drawing.Imaging.EncoderParameters 1; `$p.Param[0]=New-Object Drawing.Imaging.EncoderParameter([Drawing.Imaging.Encoder]::Quality, [long]85); " +
        "`$b.Save('$jpg', `$c, `$p); `$b.Dispose(); `$i.Dispose()"
    powershell.exe -NoProfile -Command $ps
    Write-Host "  shot $jpg"
}
function Note([string]$Title, $Lines) {
    Add-Content $Text "== $Title"
    $Lines | ForEach-Object { Add-Content $Text "  $_" }
}
function Get-LogSince([int]$Since, [string]$Pattern) {
    Get-Content $log | Select-Object -Skip $Since | Select-String $Pattern | ForEach-Object { $_.Line -replace '^\[[^\]]*\] \[[^\]]*\]: ', '' }
}
function Mark { (Get-Content $log).Count }
function C([string]$Line) { Invoke-Console $Line }
function Pedestal([string]$Horn) {
    C 'clear sc_a' | Out-Null
    C "give sc_a $Horn 1" | Out-Null
    C "tp sc_a $($ax + 0.5) $ay $($az - 2.5) 0 30" | Out-Null
    Start-Sleep -Milliseconds 600
    Invoke-Bot $bot "select $Horn" | Out-Null
    Start-Sleep -Milliseconds 400
    Invoke-Bot $bot "interact $ax $($ay + 1) $az" | Out-Null
    Start-Sleep -Seconds 2
}

Set-Content $Text "survival checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    C 'tick unfreeze' | Out-Null
    C 'sc arena' | Out-Null
    C 'sc clear' | Out-Null
    C 'gamemode creative sc_a' | Out-Null
    C 'gamerule advance_time false' | Out-Null
    C 'time set noon' | Out-Null

    if (Scene 'recipes') {
        $ids = 'altar_slab', 'altar_core', 'altar_pedestal', 'altar_socket', 'altar_pillar', 'altar_brazier', 'altar_obelisk',
            'banner_weight', 'banner_mercy', 'banner_horde', 'banner_calm', 'banner_haste', 'banner_rest', 'banner_resilience',
            'banner_kin_undead', 'banner_kin_beasts', 'banner_kin_slimes', 'banner_kin_flyers', 'banner_kin_water',
            'rotten_horn', 'grave_horn', 'storm_horn', 'abyss_horn', 'hunter_book'
        $lines = foreach ($id in $ids) { "$id -> $((C "recipe give sc_a soulscraft:$id") -join ' ')" }
        Note 'recipes (recipe give)' $lines
    }

    if (Scene 'shards') {
        C 'kill @e[type=minecraft:item,x=-500,y=101,z=500,distance=..40]' | Out-Null
        # The bot first: mobs summoned into a chunk nobody holds are gone before the blows land.
        C "tp sc_a $($ax + 5) $ay $($az - 13)" | Out-Null
        Start-Sleep -Seconds 3
        $mark = Mark
        for ($i = 0; $i -lt 10; $i++) {
            C "summon soulscraft:revenant $($ax + 0.5 + $i) $ay $($az - 10.5)" | Out-Null
        }
        Start-Sleep -Seconds 2
        for ($i = 0; $i -lt 10; $i++) {
            C "damage @e[type=soulscraft:revenant,limit=1,sort=nearest,x=$($ax + 0.5 + $i),y=$ay,z=$($az - 10.5)] 1000 minecraft:player_attack by sc_a" | Out-Null
            Start-Sleep -Milliseconds 300
        }
        Start-Sleep -Seconds 3
        $drops = @(Get-LogSince $mark 'soulscraft: shard drop')
        $credits = @(Get-LogSince $mark 'soulscraft: book credit mob=revenant')
        Note "shards: $($credits.Count) credited kills, $($drops.Count) drops" $drops
        Invoke-Bot $bot "look-at $($ax + 5) $ay $($az - 10.5)" | Out-Null
        Start-Sleep -Seconds 1
        Save-Shot $bot 'shards_on_the_ground'
    }

    if (Scene 'events') {
        foreach ($e in @(@{ horn = 'soulscraft:grave_horn'; level = 2; event = 'grave_night' },
                         @{ horn = 'soulscraft:storm_horn'; level = 3; event = 'claw_storm' },
                         @{ horn = 'soulscraft:abyss_horn'; level = 4; event = 'last_stand' })) {
            # One level too low: the horn sits on the pedestal, no event.
            C "sc altar $($e.level - 1) banners" | Out-Null
            Start-Sleep -Seconds 2
            $mark = Mark
            Pedestal $e.horn
            $low = @(Get-LogSince $mark "soulscraft: event start")
            C 'sc event stop' | Out-Null
            # Its own level: the event starts.
            C "sc altar $($e.level) banners" | Out-Null
            Start-Sleep -Seconds 2
            $mark = Mark
            Pedestal $e.horn
            $start = @(Get-LogSince $mark "soulscraft: event start")
            Note "$($e.event): level $($e.level - 1) starts $($low.Count), level $($e.level) starts $($start.Count)" ($low + $start)
            if ($e.level -eq 2) {
                Start-Sleep -Seconds 6
                C "tp sc_a $($ax - 9.5) $($ay + 7) $($az - 9.5)" | Out-Null
                Invoke-Bot $bot "look-at $($ax + 0.5) $ay $($az + 0.5)" | Out-Null
                Start-Sleep -Seconds 2
                Save-Shot $bot 'grave_night_wave'
            }
            C 'sc event stop' | Out-Null
            C 'sc clear' | Out-Null
        }
    }

    if (Scene 'hardcore') {
        C 'sc altar 1 banners' | Out-Null
        Start-Sleep -Seconds 2
        C 'gamemode survival sc_a' | Out-Null
        Pedestal 'soulscraft:rotten_horn'
        C "tp sc_a $($ax + 0.5) $ay $($az - 4.5)" | Out-Null
        Start-Sleep -Seconds 3
        $mark = Mark
        C 'kill sc_a' | Out-Null
        Start-Sleep -Seconds 3
        $state = Invoke-Bot $bot 'state' | ConvertFrom-Json
        Note 'hardcore guard' (@(Get-LogSince $mark 'soulscraft: event (spared|out)|died|was slain|fell') + "bot: health $($state.player.health) creative $($state.creative) screen $($state.screen) at $($state.player.position -join ',')")
        Save-Shot $bot 'spared_outside_the_zone'
        C 'sc event stop' | Out-Null
        C 'sc altar 0' | Out-Null
        C 'gamemode creative sc_a' | Out-Null
        C 'effect clear sc_a' | Out-Null
    }

    if (Scene 'natural') {
        # Far from the lit arena, on generated ground: the bot in survival, unhurt, at midnight.
        C 'sc clear' | Out-Null
        C 'time set midnight' | Out-Null
        C 'spreadplayers -1500 1500 0 20 false sc_a' | Out-Null
        C 'gamemode survival sc_a' | Out-Null
        C 'effect give sc_a minecraft:resistance infinite 4 true' | Out-Null
        C 'effect give sc_a minecraft:regeneration infinite 2 true' | Out-Null
        $mark = Mark
        Start-Sleep -Seconds 90
        $adopted = @(Get-LogSince $mark 'soulscraft: adopt mob=\S+ .*fresh=false')
        $lurkers = @(Get-LogSince $mark 'soulscraft: natural lurker')
        $biome = (Invoke-Bot $bot 'state' | ConvertFrom-Json).player.biome
        Note "natural: $($adopted.Count) SoulsCraft mobs adopted in 90 s (biome $biome), lurkers $($lurkers.Count)" ($adopted + $lurkers)
        Save-Shot $bot 'night_natural'
        C 'gamemode creative sc_a' | Out-Null
        C 'effect clear sc_a' | Out-Null
        C 'time set noon' | Out-Null
    }
} finally {
    C 'tick unfreeze' | Out-Null
    C 'gamerule advance_time true' | Out-Null
    C 'sc event stop' | Out-Null
    C 'sc clear' | Out-Null
    Stop-Bot $bot
}
Write-Host "Evidence in $Out"

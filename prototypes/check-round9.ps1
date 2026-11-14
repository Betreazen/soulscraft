# Round 9's in-game checks of the altar and its event (iteration 2, prototypes/p6-altar.md) with the
# bot sc_a on the joint server:
#   1. the altar at each level (/sc altar N banners) with the ghost of the next ring (an altar
#      block in hand) and the banners;
#   2. the Rotten Horn floating over the pedestal (level 0: the event does not start);
#   3. the altar's menu (right click on the core, empty hand);
#   4. the test event on a level-3 altar: the spawn points shown, the braziers lit, the boss bar,
#      zombies draining the altar with their beams;
#   5. the self-test's breach and curse scenarios watched live (soulscraft_selftest run …): the
#      brute breaking through a cobblestone ring, the walls put back, a zombie cursing through
#      bedrock.
# Shots go to prototypes/evidence/round9/ (JPEG 960 wide; the PNGs stay in the scratchpad).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"   (and close after)
param([switch]$WatchOnly)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\round9'
$Raw = Join-Path $script:Scratch 'round9'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
$Text = Join-Path $Out 'console.txt'
# `/sc altar` from the console builds 6 blocks south of the arena's centre (tools.rs ARENA).
$ax = -500; $ay = 101; $az = 506
# The self-test's site (selftest_altar.rs CORE).
$sx = -700; $sy = 101; $sz = 700

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

function Wait-Log([int]$Since, [string]$Pattern, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        $hit = Get-LogSince $Since $Pattern | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 500
    }
    $null
}

Set-Content $Text "round 9 checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    if (-not $WatchOnly) {
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'sc arena' | Out-Null
    Invoke-Console 'sc clear' | Out-Null
    Invoke-Console 'gamemode creative sc_a' | Out-Null
    Invoke-Console 'clear sc_a' | Out-Null
    Invoke-Console 'give sc_a soulscraft:altar_slab 1' | Out-Null
    Invoke-Console 'give sc_a soulscraft:rotten_horn 2' | Out-Null
    Start-Sleep -Seconds 1

    # 1. Each level with the ghost of the next ring (the slab in hand) and the banners.
    foreach ($n in 1, 2, 3, 4) {
        $mark = (Get-Content $log).Count
        Invoke-Console "sc altar $n banners" | Out-Null
        Start-Sleep -Seconds 2
        Note "level $n" (Get-LogSince $mark 'soulscraft: altar core')
        Invoke-Console "tp sc_a $($ax - 7.5) $($ay + 5) $($az - 7.5) -45 32" | Out-Null
        Start-Sleep -Milliseconds 500
        Invoke-Bot $bot "look-at $($ax + 0.5) $($ay + 1) $($az + 0.5)" | Out-Null
        Invoke-Bot $bot 'select soulscraft:altar_slab' | Out-Null
        Start-Sleep -Seconds 3
        Save-Shot $bot "altar_level$n"
    }
    Invoke-Console "tp sc_a $($ax + 0.5) $($ay + 1) $($az - 5.5) 0 25" | Out-Null
    Start-Sleep -Milliseconds 500
    Invoke-Bot $bot "look-at $($ax + 0.5) $($ay + 1.5) $($az + 0.5)" | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'altar_level4_banners_close'

    # 2. The horn on the pedestal of a level-0 altar (only the core and the pedestal).
    Invoke-Console 'sc altar 0' | Out-Null
    Start-Sleep -Seconds 1
    Invoke-Console "tp sc_a $($ax + 0.5) $ay $($az - 2.5) 0 30" | Out-Null
    Invoke-Bot $bot 'select soulscraft:rotten_horn' | Out-Null
    Start-Sleep -Seconds 1
    $mark = (Get-Content $log).Count
    Invoke-Bot $bot "interact $ax $($ay + 1) $az" | Out-Null
    Start-Sleep -Seconds 2
    Note 'pedestal' (Get-LogSince $mark 'soulscraft: (pedestal|event)')
    Invoke-Console "tp sc_a $($ax + 0.5) $($ay + 0.5) $($az - 1.7) 0 20" | Out-Null
    Start-Sleep -Milliseconds 500
    Invoke-Bot $bot "look-at $($ax + 0.5) $($ay + 2.1) $($az + 0.5)" | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'pedestal_horn'

    # 3. The menu: an empty hand on the core of a level-2 altar with banners.
    Invoke-Console 'sc altar 2 banners' | Out-Null
    Start-Sleep -Seconds 1
    Invoke-Bot $bot 'select 9' | Out-Null
    Invoke-Console "tp sc_a $($ax + 0.5) $ay $($az - 2.5) 0 40" | Out-Null
    Start-Sleep -Seconds 1
    Invoke-Bot $bot "interact $ax $ay $az" | Out-Null
    Start-Sleep -Seconds 2
    Note 'menu screen' (Invoke-Bot $bot 'screen')
    Save-Shot $bot 'menu'
    Invoke-Bot $bot 'key esc tap' | Out-Null
    Start-Sleep -Seconds 1

    # 4. The test event on a level-3 altar, the bot in its zone (creative: the zombies' blows do
    #    nothing), watching from above.
    Invoke-Console 'sc altar 3 banners' | Out-Null
    Start-Sleep -Seconds 1
    Invoke-Console "tp sc_a $($ax - 9.5) $($ay + 7) $($az - 9.5) -45 30" | Out-Null
    Start-Sleep -Milliseconds 500
    Invoke-Bot $bot "look-at $($ax + 0.5) $ay $($az + 0.5)" | Out-Null
    $mark = (Get-Content $log).Count
    Invoke-Console 'sc event start' | Out-Null
    Start-Sleep -Milliseconds 1500
    Save-Shot $bot 'event_telegraph'
    $spawn = Wait-Log $mark 'soulscraft: event spawn' 10
    Note 'event start' (Get-LogSince $mark 'soulscraft: event (start|telegraph|spawn)')
    Start-Sleep -Seconds 2
    Save-Shot $bot 'event_wave_braziers_lit'
    $drain = Wait-Log $mark 'soulscraft: event drain' 60
    Start-Sleep -Seconds 2
    Save-Shot $bot 'event_drain_beams'
    Invoke-Console "tp sc_a $($ax + 0.5) $($ay + 3) $($az - 6.5) 0 20" | Out-Null
    Start-Sleep -Milliseconds 500
    Invoke-Bot $bot "look-at $($ax + 0.5) $($ay + 1) $($az + 0.5)" | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'event_drain_close'
    Note 'event drain' (Get-LogSince $mark 'soulscraft: event (drain|lost|won)')
    Invoke-Console 'sc event stop' | Out-Null
    Start-Sleep -Seconds 1

    }

    # 5. The self-test's breach and curse scenarios, live on their site. The bot watches as a
    #    spectator (not a participant, so the wave goes for the altar), from the south, outside the
    #    cobblestone ring. The world is frozen at the breach and stepped past the blow, so the shot
    #    shows the opening before the scenario puts the walls back.
    Invoke-Console 'gamemode spectator sc_a' | Out-Null
    Invoke-Console "tp sc_a $($sx + 0.5) $($sy + 6) $($sz + 17.5) 180 25" | Out-Null
    Start-Sleep -Milliseconds 500
    Invoke-Bot $bot "look-at $($sx + 0.5) $($sy + 1) $($sz + 5.5)" | Out-Null
    $mark = (Get-Content $log).Count
    # Slow time, so the freeze lands inside the brute's swing (16 ticks to the blow).
    Invoke-Console 'tick rate 5' | Out-Null
    Invoke-Console 'soulscraft_selftest run defense_breach' | Out-Null
    $hit = Wait-Log $mark 'soulscraft: event breach' 60
    Invoke-Console 'tick freeze' | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'breach_swing'
    # Step a tick at a time until the blow lands (16 ticks into the swing), then look.
    for ($k = 0; $k -lt 40 -and -not (Get-LogSince $mark 'soulscraft: event broke'); $k++) {
        Invoke-Console 'tick step 1' | Out-Null
        Start-Sleep -Milliseconds 250
    }
    Start-Sleep -Seconds 2
    Save-Shot $bot 'breach_opening'
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'tick rate 20' | Out-Null
    # The walls come back as the scenario ends, and its clean-up clears the site in the same tick:
    # the restore is the scenario's check (and the log), not a shot.
    $restored = Wait-Log $mark 'soulscraft: event restored' 20
    Note 'breach' (Get-LogSince $mark 'soulscraft: (event (breach|broke|restored|lost)|selftest (PASS|FAIL))')
    Wait-Log $mark 'soulscraft: selftest done' 30 | Out-Null

    $mark = (Get-Content $log).Count
    Invoke-Console 'tick rate 5' | Out-Null
    Invoke-Console 'soulscraft_selftest run defense_curse' | Out-Null
    $curse = Wait-Log $mark 'soulscraft: event curse' 60
    # The beam reaches clients with the altar's view, every 10 ticks: freeze after one went out.
    Start-Sleep -Milliseconds 800
    Invoke-Console 'tick freeze' | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'curse_beam'
    Invoke-Console 'tick unfreeze' | Out-Null
    Note 'curse' (Get-LogSince $mark 'soulscraft: (event (curse|lost)|selftest (PASS|FAIL))')
    Wait-Log $mark 'soulscraft: selftest done' 30 | Out-Null
} finally {
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'tick rate 20' | Out-Null
    Stop-Bot $bot
    Invoke-Console 'sc clear' | Out-Null
}

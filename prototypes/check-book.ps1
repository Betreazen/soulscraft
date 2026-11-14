# The hunter's book in game (П9, prototypes/p9-book.md) with the bot sc_a on the joint server: the
# book's window locked and opened by stages, the action-bar line of a new stage, the altar and
# events sections, and the progress kept over a relog (the player's persistent data).
# Shots go to prototypes/evidence/book/ (JPEG 960 wide; the PNGs stay in the scratchpad).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft book agent"   (stop, close after)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\book'
$Raw = Join-Path $script:Scratch 'book'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$Text = Join-Path $Out 'console.txt'
$ax = -500; $ay = 101; $az = 500

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
function C([string]$Line) { $o = Invoke-Console $Line; Note $Line $o }

# The window's layout (book_client.rs `layout`) for a GUI of $W × $H.
function Get-Layout([double]$W, [double]$H) {
    $pw = [Math]::Min($W - 16, 440); $ph = [Math]::Min($H - 16, 260)
    $px = ($W - $pw) / 2; $py = ($H - $ph) / 2
    $top = $py + 26
    [pscustomobject]@{ Px = $px; Py = $py; Top = $top; Tab = { param($i) @(($px + 6 + $i * 68 + 32), ($py + 13)) }.GetNewClosure()
        Row = { param($i) @(($px + 6 + 50), ($top + $i * 11 + 5)) }.GetNewClosure()
        Clip = { param($i) @(($px + 6 + 104 + 6 + 52), ($top + 108 + $i * 12 + 5)) }.GetNewClosure() }
}
function Click($Bot, $At) { Invoke-Bot $Bot "click-at $([int]$At[0]) $([int]$At[1])" | Out-Null; Start-Sleep -Milliseconds 700 }
function Open-Book($Bot) {
    Invoke-Bot $Bot 'select 1' | Out-Null
    Invoke-Bot $Bot 'use-item' | Out-Null
    Start-Sleep -Seconds 2
}

Set-Content $Text "book checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    C 'tick unfreeze'
    C 'sc arena'
    C 'sc clear'
    C 'gamemode creative sc_a'
    C 'clear sc_a'
    C 'time set noon'
    C 'weather clear'
    C "tp sc_a $($ax + 0.5) $ay $($az + 0.5) 0 0"
    C 'item replace entity sc_a hotbar.0 with soulscraft:hunter_book'
    C 'sc book sc_a reset'
    Start-Sleep -Seconds 1

    # Locked: every entry «???», the first one's model a black silhouette.
    Open-Book $bot
    Note 'screen' (Invoke-Bot $bot 'screen' | Out-String)
    Save-Shot $bot 'book_locked'

    Invoke-Bot $bot 'key esc tap' | Out-Null

    # A new stage: the action-bar line.
    C 'sc book sc_a kill revenant 1'
    Start-Sleep -Milliseconds 600
    Save-Shot $bot 'stage_message_killed'
    C 'sc book sc_a kill revenant 4'
    Start-Sleep -Milliseconds 600
    Save-Shot $bot 'stage_message_studied'
    C 'sc book sc_a meet stalker'
    C 'sc book sc_a kill brute 8'
    Start-Sleep -Seconds 1

    # The pages: the studied revenant (first attack playing, its strip), the met stalker, the
    # brute (elite, expert at 8), a locked one, then the altar and events sections.
    Open-Book $bot
    # The window logs its screen rectangle on its first draw (book_client.rs): the GUI size.
    $w = 480; $h = 270
    $log = Join-Path $bot.Dir 'logs\latest.log'
    $line = Select-String -Path $log -Pattern 'book window drawn .*screen=\[([\d.]+), ([\d.]+), ([\d.]+), ([\d.]+)\]' | Select-Object -Last 1
    if ($line) { $w = [double]$line.Matches[0].Groups[3].Value; $h = [double]$line.Matches[0].Groups[4].Value }
    Note 'gui' "$w x $h"
    $l = Get-Layout $w $h
    Save-Shot $bot 'book_revenant_studied'
    Start-Sleep -Milliseconds 900
    Save-Shot $bot 'book_revenant_studied_later'
    Click $bot (& $l.Clip 0)
    Save-Shot $bot 'book_revenant_idle'
    Click $bot (& $l.Row 1)
    Save-Shot $bot 'book_stalker_met'
    Click $bot (& $l.Row 3)
    Save-Shot $bot 'book_brute_expert'
    Click $bot (& $l.Row 4)
    Save-Shot $bot 'book_shrike_locked'
    Click $bot (& $l.Row 7)
    Save-Shot $bot 'book_vanilla_locked'
    Click $bot (& $l.Tab 1)
    Save-Shot $bot 'book_altar_locked'
    Click $bot (& $l.Tab 2)
    Save-Shot $bot 'book_events_locked'
    Invoke-Bot $bot 'key esc tap' | Out-Null

    # The altar: an altar block in hand opens its base; a level-2 altar built near opens levels.
    C 'item replace entity sc_a hotbar.1 with soulscraft:altar_core'
    Invoke-Bot $bot 'select 2' | Out-Null
    Start-Sleep -Seconds 2
    C 'item replace entity sc_a hotbar.2 with soulscraft:banner_horde'
    Invoke-Bot $bot 'select 3' | Out-Null
    Start-Sleep -Seconds 2
    C 'sc altar 2'
    Start-Sleep -Seconds 3
    Open-Book $bot
    Click $bot (& $l.Tab 1)
    Save-Shot $bot 'book_altar_opened'
    Invoke-Bot $bot 'key esc tap' | Out-Null
    Note 'book before relog' (Invoke-Console 'sc book sc_a')

    # A relog: the same book from the player's persistent data.
    Stop-Bot $bot
    Start-Sleep -Seconds 3
    $bot = Start-Bot sc_a
    Start-Sleep -Seconds 3
    Note 'book after relog' (Invoke-Console 'sc book sc_a')
    Open-Book $bot
    Save-Shot $bot 'book_after_relog'
    Invoke-Bot $bot 'key esc tap' | Out-Null
} finally {
    C 'gamemode creative sc_a'
    C 'clear sc_a'
    C 'sc book sc_a reset'
    Stop-Bot $bot
    C 'sc clear'
}

# The diorama plaque's text (09-trophies.md, «Подставка»): the mob's name and the bot's kills from
# its hunter's book, on every diorama, close up by day and at night; then one revenant killed by
# the bot (`damage … by sc_a`) and the revenant's plaque again, which should count it.
# Shots go to prototypes/evidence/plaque/ (JPEG 960 wide; the PNGs stay in the scratchpad).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 serve -Owner "soulscraft session"   (and .\joint.ps1 stop after)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\plaque'
$Raw = Join-Path $script:Scratch 'plaque'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
$Text = Join-Path $Out 'console.txt'
$Mobs = 'revenant', 'stalker', 'marksman', 'brute', 'shrike', 'lurker', 'bog_slime'
# The arena (tools.rs ARENA): a flat lit floor at y 100 round (-500, 500).
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
function Get-LogSince([int]$Since, [string]$Pattern) {
    Get-Content $log | Select-Object -Skip $Since | Select-String $Pattern | ForEach-Object { $_.Line -replace '^\[[^\]]*\] \[[^\]]*\]: ', '' }
}
function Mark { (Get-Content $log).Count }
function C([string]$Line) { Invoke-Console $Line | Out-Null }
function View([double]$X, [double]$Y, [double]$Z, [double]$LX, [double]$LY, [double]$LZ) {
    C "tp sc_a $X $Y $Z"
    Start-Sleep -Milliseconds 400
    Invoke-Bot $bot "look-at $LX $LY $LZ" | Out-Null
    Start-Sleep -Milliseconds 1500
}
function Plaque([string]$Mob, [string]$Name) {
    $x = $ax - 6 + 2 * [array]::IndexOf($Mobs, $Mob)
    # On a stone a block high: the plaque (y ay + 1.1, its face at z = az - 7) near eye height.
    View ($x + 0.5) $ay ($az - 6.1) ($x + 0.5) ($ay + 1.09) ($az - 7.0)
    Save-Shot $bot $Name
}

Set-Content $Text "plaque checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    C 'tick unfreeze'
    C 'sc arena'
    C 'sc clear'
    C 'gamemode creative sc_a'
    C 'clear sc_a'
    C 'gamerule doDaylightCycle false'
    C 'time set noon'
    C 'weather clear'
    C "fill $($ax - 12) $ay $($az - 14) $($ax + 12) $($ay + 6) $($az - 4) air"
    for ($i = 0; $i -lt $Mobs.Count; $i++) {
        C "setblock $($ax - 6 + 2 * $i) $ay $($az - 8) minecraft:stone"
        C "setblock $($ax - 6 + 2 * $i) $($ay + 1) $($az - 8) soulscraft:$($Mobs[$i])_trophy[facing=south]"
    }
    Start-Sleep -Seconds 2
    View ($ax + 0.5) ($ay + 1.0) ($az - 3.5) ($ax + 0.5) ($ay + 1.2) ($az - 7.5)
    Save-Shot $bot 'row_day'
    foreach ($m in $Mobs) { Plaque $m "plaque_$($m)_day" }
    C 'time set midnight'
    Start-Sleep -Seconds 2
    Plaque 'brute' 'plaque_brute_night'
    C 'time set noon'
    # One revenant killed by the bot: the book credits it, the client gets the progress.
    $mark = Mark
    C "summon soulscraft:revenant $($ax + 0.5) $ay $($az - 2.5)"
    Start-Sleep -Seconds 2
    C "damage @e[type=soulscraft:revenant,limit=1,sort=nearest,x=$ax,y=$ay,z=$($az - 2)] 1000 minecraft:player_attack by sc_a"
    Start-Sleep -Seconds 3
    Note 'book credit' (Get-LogSince $mark 'soulscraft: book (credit|message)')
    Plaque 'revenant' 'plaque_revenant_after_kill'
} finally {
    C 'gamerule doDaylightCycle true'
    Stop-Bot $bot
    C 'sc clear'
}

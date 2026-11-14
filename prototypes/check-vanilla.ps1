# Vanilla telegraphs (П8, prototypes/p8-vanilla.md) with the bot sc_a on the joint server: a few
# changed vanilla mobs forced into a move by `/sc force` (the nearest SoulsCraft or changed vanilla
# mob to the arena), shot in the windup from the front: the `aggressive` pose, the cue particles,
# the `/sc debug` label over the mob. Each mob stands still (NoAI) facing the camera.
# Shots go to prototypes/evidence/vanilla/ (JPEG 960 wide; the PNGs stay in the scratchpad).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"   (stop, close after)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\vanilla'
$Raw = Join-Path $script:Scratch 'vanilla'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$Text = Join-Path $Out 'console.txt'
$ax = -500; $ay = 101; $az = 500
# Type, move, extra NBT, delay from the force to the shot (ms; a tick is 50 ms).
$Moves = @(
    @('minecraft:zombie', 'lunge', '', 250),
    @('minecraft:vindicator', 'overhead', '', 300),
    @('minecraft:hoglin', 'ram', '', 250),
    @('minecraft:spider', 'pounce', '', 200),
    @('minecraft:witch', 'potion_throw', '', 300),
    @('minecraft:piglin_brute', 'heavy_strike', '', 300)
)

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

Set-Content $Text "vanilla telegraph checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    C 'tick unfreeze'
    C 'sc arena'
    C 'sc clear'
    C 'time set noon'
    C 'weather clear'
    C 'gamemode spectator sc_a'
    C 'sc debug on'
    Invoke-Bot $bot 'key f1 tap' | Out-Null
    C "tp sc_a $($ax + 0.5) $($ay + 0.6) $($az + 6.5) 180 8"
    foreach ($m in $Moves) {
        $kind, $move, $extra, $delay = $m
        $name = ($kind -replace 'minecraft:', '') + "_$move"
        C "summon $kind $($ax + 0.5) $ay $($az + 2.5) {NoAI:1b,PersistenceRequired:1b,Rotation:[0f,0f],Tags:[`"soulscraft.p=0.00`",`"soulscraft.fresh`"]$extra}"
        Start-Sleep -Seconds 2
        Save-Shot $bot "${name}_rest"
        C "sc force $move"
        Start-Sleep -Milliseconds $delay
        Save-Shot $bot "${name}_windup"
        Start-Sleep -Seconds 2
        C "kill @e[type=$kind,x=$ax,y=$ay,z=$az,distance=..10]"
        Start-Sleep -Milliseconds 500
    }
} finally {
    C 'sc debug off'
    C 'gamemode creative sc_a'
    Stop-Bot $bot
    C 'sc clear'
}

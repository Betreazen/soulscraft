# Round 8's in-game checks with the bot sc_a on the joint server (decisions 1–3 of round 8):
#   1. hitboxes: all seven mobs in a row, frozen, with F3+B on (each mob's own box);
#   2. the bleed: the stalker's double slash on the bot in survival (Resistance IV), its effects and
#      the HUD with soulscraft:bleeding's icon (textures/mob_effect/bleeding.png);
#   3. P: `pct_selftest summary` (Path-of-craft-tree's own print) against `/sc power`, then a
#      `/sc pct` test summary, and a mob spawned without a P tag takes the bot's P (Г1, Г5).
# Screenshots and the console's answers go to prototypes/evidence/round8/ (JPEG, 960 wide).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"   (and close after)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\round8'
$Raw = Join-Path $script:Scratch 'round8'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
$Text = Join-Path $Out 'console.txt'
$cx = -499.5; $cy = 101; $cz = 500.5

# A shot of the bot's view as a JPEG 960 wide (System.Drawing of Windows PowerShell).
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

# The SoulsCraft lines the server logged since a mark.
function Get-LogSince([int]$Since, [string]$Pattern) {
    Get-Content $log | Select-Object -Skip $Since | Select-String $Pattern | ForEach-Object { $_.Line -replace '^\[[^\]]*\] \[[^\]]*\]: ', '' }
}

Set-Content $Text "round 8 checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    Invoke-Console 'sc arena' | Out-Null
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'sc clear' | Out-Null
    Invoke-Console 'gamemode creative sc_a' | Out-Null

    # 1. Hitboxes: the seven in a row 7 blocks south of the bot, looking at them from the north.
    $ids = 'revenant', 'stalker', 'marksman', 'brute', 'shrike', 'lurker', 'bog_slime'
    for ($i = 0; $i -lt $ids.Count; $i++) {
        $x = $cx - 7.5 + $i * 2.5
        $tags = "Tags:[`"soulscraft.p=0.00`",`"soulscraft.fresh`"" + $(if ($ids[$i] -eq 'bog_slime') { ',"soulscraft.size=4"' } else { '' }) + ']'
        Invoke-Console "summon soulscraft:$($ids[$i]) $x $cy $($cz + 4) {Rotation:[180f,0f],$tags,PersistenceRequired:1b,NoAI:1b}" | Out-Null
    }
    Start-Sleep -Seconds 1
    Invoke-Console 'tick freeze' | Out-Null
    Invoke-Console "tp sc_a $cx $cy $($cz - 4) 0 10" | Out-Null
    Start-Sleep -Seconds 3
    Invoke-Bot $bot 'key f3 down' | Out-Null
    Invoke-Bot $bot 'key b tap' | Out-Null
    Invoke-Bot $bot 'key f3 up' | Out-Null
    Start-Sleep -Seconds 1
    $debug = (Invoke-Bot $bot 'state' | ConvertFrom-Json).debug
    Note 'F3+B' "hitboxes shown: $($debug.hitboxes)"
    Save-Shot $bot 'hitboxes_all'
    Invoke-Console "tp sc_a $($cx + 0.5) $cy $($cz - 1) 0 0" | Out-Null
    Start-Sleep -Seconds 2
    Save-Shot $bot 'hitboxes_brute_close'
    Invoke-Bot $bot 'key f3 down' | Out-Null
    Invoke-Bot $bot 'key b tap' | Out-Null
    Invoke-Bot $bot 'key f3 up' | Out-Null
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'sc clear' | Out-Null

    # 2. The bleed: a stalker's double slash on the bot.
    Invoke-Console "tp sc_a $cx $cy $cz 180 10" | Out-Null
    Invoke-Console 'gamemode survival sc_a' | Out-Null
    # Resistance IV (80 %): V would stop the blow entirely, and a blow that does nothing gives no bleed.
    Invoke-Console 'effect give sc_a minecraft:resistance infinite 3 true' | Out-Null
    $since = (Get-Content $log).Count
    Invoke-Console "summon soulscraft:stalker $cx $cy $($cz - 1.8) {Rotation:[0f,0f],Tags:[`"soulscraft.p=0.00`",`"soulscraft.fresh`"],PersistenceRequired:1b}" | Out-Null
    Start-Sleep -Seconds 1
    Invoke-Console 'sc force double_slash' | Out-Null
    # The bleed lasts 3 s from the blow (mobs.json): shot and effects at once after it lands.
    Start-Sleep -Milliseconds 1300
    Save-Shot $bot 'bleeding_hud'
    Note 'bot effects' (Invoke-Bot $bot 'effects')
    Note 'stalker strikes' (Get-LogSince $since 'soulscraft: (strike|phase) mob=stalker')
    # The inventory's effect list, with the bleed given by command for long enough to open it.
    Invoke-Console 'effect give sc_a soulscraft:bleeding 20 0' | Out-Null
    Invoke-Bot $bot 'key e tap' | Out-Null
    Start-Sleep -Seconds 1
    Save-Shot $bot 'bleeding_inventory'
    Invoke-Bot $bot 'key esc tap' | Out-Null
    Invoke-Console 'sc clear' | Out-Null
    Invoke-Console 'effect clear sc_a' | Out-Null
    Invoke-Console 'gamemode creative sc_a' | Out-Null

    # 3. P: PCT's summary as PCT prints it, against SoulsCraft's reading; then a test summary.
    Start-Sleep -Seconds 2
    $since = (Get-Content $log).Count
    Note 'pct_selftest summary (real, after login)' (Invoke-Console 'pct_selftest summary')
    Invoke-Console 'sc power' | Out-Null
    Start-Sleep -Seconds 1
    Note '/sc power (real summary)' (Get-LogSince $since 'soulscraft: (sc_a|pct player)')
    $since = (Get-Content $log).Count
    Invoke-Console 'sc pct sc_a 60 12 30' | Out-Null
    Start-Sleep -Seconds 3
    Note 'pct_selftest summary (after /sc pct sc_a 60 12 30)' (Invoke-Console 'pct_selftest summary')
    Invoke-Console 'sc power' | Out-Null
    Start-Sleep -Seconds 1
    # A mob without a P tag takes the P of the players near it (Г5): here the bot's.
    Invoke-Console "summon soulscraft:revenant $cx $cy $($cz + 6) {PersistenceRequired:1b}" | Out-Null
    Start-Sleep -Seconds 2
    Note '/sc power and the adopted P (test summary)' (Get-LogSince $since 'soulscraft: (sc_a|pct player|sc pct|adopt mob=revenant)')
    Invoke-Console 'sc clear' | Out-Null
    $since = (Get-Content $log).Count
    Invoke-Console 'sc pct sc_a off' | Out-Null
    Start-Sleep -Seconds 3
    Invoke-Console 'sc power' | Out-Null
    Start-Sleep -Seconds 1
    Note '/sc power after /sc pct sc_a off (gear only)' (Get-LogSince $since 'soulscraft: (sc_a|pct player|sc pct)')
} finally {
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'sc clear' | Out-Null
    Invoke-Console 'gamemode creative sc_a' | Out-Null
    Stop-Bot $bot
}
Write-Host "Evidence in $Out"

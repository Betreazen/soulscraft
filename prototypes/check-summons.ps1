# The summons slice in game (docs/design/11-summons-rewards.md) with the bot sc_a on the joint server:
#   1. chests: in the bench self-test chest_loot (`loot give` drops nothing on this engine);
#   2. tier items: the Revenant's Blade, the Brute's Maul and the Abyss Heart in hand, the
#      player's attributes read back (armour, attack damage, max health), shots of each in hand;
#   3. the brute still holds its 3D maul (the item model falls back to it without a holder).
# Shots and the console's answers go to prototypes/evidence/summons/.
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft session"   (and stop, close after)
# Run through `pwsh -Command "& .\prototypes\check-summons.ps1 -Only items,brute"` for scene lists.
param([string[]]$Only)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\summons'
$Raw = Join-Path $script:Scratch 'summons'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$Text = Join-Path $Out 'console.txt'
function Scene([string]$Name) { -not $Only -or $Only -contains $Name }
function C([string]$Line) { Invoke-Console $Line }
function Note([string]$Title, $Lines) {
    Add-Content $Text "== $Title"
    $Lines | ForEach-Object { Add-Content $Text "  $_" }
}
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
# How many of `item` the bot holds: `clear <player> <item> 0` only counts.
function Count-Item([string]$Item) {
    $answer = (C "clear sc_a $Item 0") -join ' '
    if ($answer -match '(\d+)') { [int]$Matches[1] } else { 0 }
}
$horns = 'soulscraft:rotten_horn', 'soulscraft:grave_horn', 'soulscraft:storm_horn', 'soulscraft:abyss_horn'
# The arena's floor (tools.rs ARENA): the bot stands on it for the hand shots.
$ax = -500; $ay = 101; $az = 500

Set-Content $Text "summons checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
# A bot that left dead comes back on the death screen: respawn first.
if ((Invoke-Bot $bot 'state' | ConvertFrom-Json).screen -match 'dead|death') { Invoke-Bot $bot 'close' | Out-Null; Start-Sleep -Seconds 2 }
try {
    C 'tick unfreeze' | Out-Null
    C 'gamemode creative sc_a' | Out-Null
    C 'gamerule advance_time false' | Out-Null
    C 'time set noon' | Out-Null
    # On the ground (a fresh world has no arena at y 101): creative all along, attributes don't care.
    C "spreadplayers $ax $az 0 1 false sc_a" | Out-Null

    # The chests: `loot give` to a player drops nothing on Horizon 7e4e421 ("Dropped 0 items", vanilla
    # tables too), so the bench self-test chest_loot rolls each tier's table with `loot spawn`.

    if (Scene 'items') {
        C 'clear sc_a' | Out-Null
        $base = @{
            armor = (C 'attribute sc_a minecraft:armor get') -join ' '
            damage = (C 'attribute sc_a minecraft:attack_damage get') -join ' '
            health = (C 'attribute sc_a minecraft:max_health get') -join ' '
        }
        Note 'bare hands' @("armor: $($base.armor)", "attack damage: $($base.damage)", "max health: $($base.health)")
        foreach ($i in @(@{ id = 'soulscraft:revenant_blade'; slot = 'weapon.mainhand' },
                         @{ id = 'soulscraft:brute_maul'; slot = 'weapon.mainhand' },
                         @{ id = 'soulscraft:abyss_heart'; slot = 'weapon.offhand' })) {
            C 'clear sc_a' | Out-Null
            C "item replace entity sc_a $($i.slot) with $($i.id)" | Out-Null
            # The Abyss Heart's health comes from the server's relic check, twice a second.
            Start-Sleep -Milliseconds 1500
            $lines = @(
                "armor: $((C 'attribute sc_a minecraft:armor get') -join ' ')",
                "attack damage: $((C 'attribute sc_a minecraft:attack_damage get') -join ' ')",
                "attack speed: $((C 'attribute sc_a minecraft:attack_speed get') -join ' ')",
                "attack knockback: $((C 'attribute sc_a minecraft:attack_knockback get') -join ' ')",
                "max health: $((C 'attribute sc_a minecraft:max_health get') -join ' ')"
            )
            Note "$($i.id) in $($i.slot)" $lines
            Invoke-Bot $bot 'look 0 20' | Out-Null
            Start-Sleep -Seconds 1
            Save-Shot $bot ("hand_" + $i.id.Split(':')[1])
        }
        C 'clear sc_a' | Out-Null
        foreach ($id in 'revenant_blade', 'brute_maul', 'abyss_heart', 'grave_horn', 'soul_shard') { C "give sc_a soulscraft:$id 1" | Out-Null }
        Invoke-Bot $bot 'open-inventory' | Out-Null
        Start-Sleep -Seconds 1
        Save-Shot $bot 'inventory_rewards'
        Invoke-Bot $bot 'key esc tap' | Out-Null
        C 'gamemode creative sc_a' | Out-Null
        C 'clear sc_a' | Out-Null
    }

    if (Scene 'brute') {
        C 'sc clear' | Out-Null
        C "execute at sc_a run summon soulscraft:brute ~ ~ ~6 {NoAI:1b}" | Out-Null
        C "execute at sc_a run tp sc_a ~3 ~ ~2" | Out-Null
        Start-Sleep -Seconds 2
        $b = (Invoke-Bot $bot 'state' | ConvertFrom-Json).player.position
        Invoke-Bot $bot "look-at $($b[0] - 3) $($b[1] + 1.5) $($b[2] + 4)" | Out-Null
        Start-Sleep -Seconds 1
        Save-Shot $bot 'brute_holds_its_maul'
        C 'sc clear' | Out-Null
    }
} finally {
    C 'tick unfreeze' | Out-Null
    C 'gamerule advance_time true' | Out-Null
    C 'gamemode creative sc_a' | Out-Null
    C 'clear sc_a' | Out-Null
    Stop-Bot $bot
}
Write-Host "Evidence in $Out"

# Round 10's in-game checks of iteration 3 (prototypes/p7-rewards.md, p4-spawn.md) with the bot sc_a
# on the joint server:
#   1. every mob's diorama in a row, by day and by night, and close up at night (glow, light,
#      particles); one diorama placed by the bot (it turns its plaque to the bot);
#   2. heads: one placed by the bot on the floor (it turns to the bot), one put on a wall by the
#      bot's click on a block's side, all seven on the floor and on a wall;
#   3. worn: a head on the bot's head, the rusty helmet and chestplate, the hoods; an armour stand
#      with the rusty pieces (mannequins draw no armour on 7e4e421: noted, not shot);
#   4. the altar's menu with banner sums (a level-3 altar with sample banners);
#   5. PCT's experience end to end: a survival kill of a revenant at P 1 (pct.xp145) with a sword
#      and with a bow, and of a size-1 bog slime at P 0 (pct.xp6): the action bar's «+X опыта»;
#   6. the spawn boost: a level-4 altar at night with the bot near, the boost's log lines (passes,
#      lists, our injections' time) with the switch on, then off.
# Shots go to prototypes/evidence/round10/ (JPEG 960 wide; the PNGs stay in the scratchpad).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"   (and close after)
param([string[]]$Parts = @('dioramas', 'heads', 'worn', 'menu', 'xp', 'boost'))
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\round10'
$Raw = Join-Path $script:Scratch 'round10'
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

function Wait-Log([int]$Since, [string]$Pattern, [int]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        $hit = Get-LogSince $Since $Pattern | Select-Object -First 1
        if ($hit) { return $hit }
        Start-Sleep -Milliseconds 500
    }
    $null
}

function Mark { (Get-Content $log).Count }
function C([string]$Line) { Invoke-Console $Line | Out-Null }
function View([double]$X, [double]$Y, [double]$Z, [double]$LX, [double]$LY, [double]$LZ) {
    C "tp sc_a $X $Y $Z"
    Start-Sleep -Milliseconds 400
    Invoke-Bot $bot "look-at $LX $LY $LZ" | Out-Null
    Start-Sleep -Milliseconds 1500
}
# The bot's action bar and recent messages (PCT's «+X опыта»).
function Bar { Invoke-Bot $bot 'messages' | Out-String }

Set-Content $Text "round 10 checks, $(Get-Date -Format s)"
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
    # Room: clear a strip north of the arena's middle for the row of dioramas.
    C "fill $($ax - 12) $ay $($az - 14) $($ax + 12) $($ay + 6) $($az - 4) air"

    if ($Parts -contains 'dioramas') {
        # 1. The row: x from -6 to +6 every 2, z = az - 8, plaques to the south (the bot).
        for ($i = 0; $i -lt $Mobs.Count; $i++) {
            C "setblock $($ax - 6 + 2 * $i) $ay $($az - 8) soulscraft:$($Mobs[$i])_trophy[facing=south]"
        }
        $mark = Mark
        Start-Sleep -Seconds 2
        View ($ax + 0.5) ($ay + 1.6) ($az - 3.5) ($ax + 0.5) ($ay + 0.4) ($az - 7.5)
        Save-Shot $bot 'dioramas_row_day'
        C 'time set midnight'
        Start-Sleep -Seconds 3
        Save-Shot $bot 'dioramas_row_night'
        for ($i = 0; $i -lt $Mobs.Count; $i++) {
            $x = $ax - 6 + 2 * $i
            View ($x + 0.5) ($ay + 0.9) ($az - 6.3) ($x + 0.5) ($ay + 0.35) ($az - 7.5)
            Save-Shot $bot "diorama_$($Mobs[$i])_night"
        }
        C 'time set noon'
        Start-Sleep -Seconds 1
        foreach ($m in 'revenant', 'brute', 'lurker') {
            $x = $ax - 6 + 2 * [array]::IndexOf($Mobs, $m)
            View ($x + 0.5) ($ay + 0.9) ($az - 6.3) ($x + 0.5) ($ay + 0.35) ($az - 7.5)
            Save-Shot $bot "diorama_$($m)_day"
        }
        Note 'diorama particles' (Get-LogSince $mark 'soulscraft: trophy particles')
        # The bot places one: it should face the bot (the bot stands south, looks north).
        C "give sc_a soulscraft:shrike_trophy 1"
        Invoke-Bot $bot 'select soulscraft:shrike_trophy' | Out-Null
        C "tp sc_a $($ax + 8.5) $ay $($az - 5.5) 180 30"
        Start-Sleep -Milliseconds 500
        Invoke-Bot $bot "place $($ax + 8) $ay $($az - 8)" | Out-Null
        Start-Sleep -Seconds 1
        Note 'placed diorama' (Invoke-Bot $bot "block $($ax + 8) $ay $($az - 8)")
    }

    if ($Parts -contains 'heads') {
        # 2. A wall of stone at z = az - 12 for the wall heads; floor heads in front of it.
        C "fill $($ax - 7) $ay $($az - 12) $($ax + 7) $($ay + 2) $($az - 12) stone"
        for ($i = 0; $i -lt $Mobs.Count; $i++) {
            $x = $ax - 6 + 2 * $i
            C "setblock $x $($ay + 1) $($az - 11) soulscraft:$($Mobs[$i])_head_mount[facing=south]"
            C "setblock $x $ay $($az - 10) soulscraft:$($Mobs[$i])_head_stand[rotation=8]"
        }
        C "fill $($ax - 7) $ay $($az - 9) $($ax + 7) $ay $($az - 4) air"
        Start-Sleep -Seconds 2
        View ($ax + 0.5) ($ay + 1.4) ($az - 6.0) ($ax + 0.5) ($ay + 0.8) ($az - 11)
        Save-Shot $bot 'heads_floor_and_wall'
        # The bot puts one on the wall (a click on the stone's south face) and one on the floor.
        C "give sc_a soulscraft:brute_head 2"
        Invoke-Bot $bot 'select soulscraft:brute_head' | Out-Null
        $mark = Mark
        C "tp sc_a $($ax + 8.5) $ay $($az - 9.5) 180 0"
        C "setblock $($ax + 8) $ay $($az - 12) stone"
        C "setblock $($ax + 8) $($ay + 1) $($az - 12) stone"
        Start-Sleep -Milliseconds 500
        Invoke-Bot $bot "look-at block $($ax + 8) $($ay + 1) $($az - 12) south" | Out-Null
        Invoke-Bot $bot "interact $($ax + 8) $($ay + 1) $($az - 12) south" | Out-Null
        Start-Sleep -Seconds 1
        Note 'wall head by the bot' ((Get-LogSince $mark 'soulscraft: trophy wall') + (Invoke-Bot $bot "block $($ax + 8) $($ay + 1) $($az - 11)"))
        C "tp sc_a $($ax + 10.5) $ay $($az - 6.5) 135 30"
        Start-Sleep -Milliseconds 500
        Invoke-Bot $bot "place $($ax + 10) $ay $($az - 9)" | Out-Null
        Start-Sleep -Seconds 1
        Note 'floor head by the bot (looking north-west, yaw 135)' (Invoke-Bot $bot "block $($ax + 10) $ay $($az - 9)")
        View ($ax + 9.5) ($ay + 1.5) ($az - 6.5) ($ax + 9) ($ay + 0.8) ($az - 10)
        Save-Shot $bot 'heads_by_the_bot'
    }

    if ($Parts -contains 'worn') {
        # 3. Third person, the bot's front.
        C "tp sc_a $($ax + 0.5) $ay $($az + 4.5) 0 0"
        # F5 twice: third person, from the front.
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        foreach ($set in @(
                @{ name = 'worn_brute_head'; head = 'soulscraft:brute_head'; chest = $null },
                @{ name = 'worn_revenant_head'; head = 'soulscraft:revenant_head'; chest = $null },
                @{ name = 'worn_rusty'; head = 'soulscraft:rusty_helmet'; chest = 'soulscraft:rusty_chestplate' },
                @{ name = 'worn_stalker_hood'; head = 'soulscraft:stalker_hood'; chest = $null },
                @{ name = 'worn_marksman_hood'; head = 'soulscraft:marksman_hood'; chest = $null },
                @{ name = 'worn_brute_helmet'; head = 'soulscraft:brute_helmet'; chest = $null })) {
            C "item replace entity sc_a armor.head with $($set.head)"
            if ($set.chest) { C "item replace entity sc_a armor.chest with $($set.chest)" } else { C 'item replace entity sc_a armor.chest with air' }
            Start-Sleep -Seconds 2
            Save-Shot $bot $set.name
        }
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        C 'item replace entity sc_a armor.head with air'
        C 'item replace entity sc_a armor.chest with air'
        C "summon minecraft:armor_stand $($ax + 3.5) $ay $($az + 2.5) {Rotation:[180f,0f],equipment:{head:{id:`"soulscraft:rusty_helmet`",count:1},chest:{id:`"soulscraft:rusty_chestplate`",count:1}}}"
        View ($ax + 3.5) ($ay + 1.4) ($az + 0.5) ($ax + 3.5) ($ay + 1.2) ($az + 2.5)
        Save-Shot $bot 'armour_stand_rusty'
    }

    if ($Parts -contains 'menu') {
        # 4. The menu of a level-3 altar with its sample banners (built 6 blocks south).
        C 'sc altar 3 banners'
        Start-Sleep -Seconds 2
        Invoke-Bot $bot 'select 9' | Out-Null
        C "tp sc_a $($ax + 0.5) $ay $($az + 3.5) 0 40"
        Start-Sleep -Seconds 1
        Invoke-Bot $bot "interact $ax $ay $($az + 6)" | Out-Null
        Start-Sleep -Seconds 2
        Note 'menu screen' (Invoke-Bot $bot 'screen')
        Save-Shot $bot 'menu_banner_sums'
        Invoke-Bot $bot 'key esc tap' | Out-Null
        Start-Sleep -Seconds 1
    }

    if ($Parts -contains 'xp') {
        # 5. PCT's experience end to end, in survival with a hard sword; strength makes one blow.
        C 'time set noon'
        C 'sc clear'
        C 'gamemode survival sc_a'
        C 'effect give sc_a minecraft:resistance 300 4 true'
        C 'effect give sc_a minecraft:strength 300 40 true'
        C 'give sc_a minecraft:netherite_sword 1'
        C 'give sc_a minecraft:bow 1'
        C 'give sc_a minecraft:arrow 16'
        $kills = @(
            @{ name = 'xp_revenant_melee'; summon = 'soulscraft:revenant'; tags = '"soulscraft.p=1.00","soulscraft.fresh"'; weapon = 'minecraft:netherite_sword' },
            @{ name = 'xp_revenant_arrow'; summon = 'soulscraft:revenant'; tags = '"soulscraft.p=1.00","soulscraft.fresh"'; weapon = 'minecraft:bow' },
            @{ name = 'xp_slime_size1'; summon = 'soulscraft:bog_slime'; tags = '"soulscraft.p=0.00","soulscraft.fresh","soulscraft.size=1"'; weapon = 'minecraft:netherite_sword' })
        foreach ($k in $kills) {
            $far = if ($k.weapon -eq 'minecraft:bow') { 6 } else { 2 }
            C "tp sc_a $($ax + 0.5) $ay $($az + 0.5) 180 0"
            C "summon $($k.summon) $($ax + 0.5) $ay $($az - $far + 0.5) {Tags:[$($k.tags)],NoAI:1b,PersistenceRequired:1b}"
            Start-Sleep -Seconds 2
            if ($k.summon -eq 'soulscraft:revenant') {
                # A revenant at P 1 has 57 health and armour: bring it down to 1 health first (out of
                # the world, no attacker), so the sword's or the arrow's hit is the killing blow.
                C "damage @e[type=$($k.summon),limit=1,sort=nearest,x=$($ax + 0.5),y=$ay,z=$($az - $far + 0.5),distance=..3] 56 minecraft:out_of_world"
                Start-Sleep -Milliseconds 300
            }
            $mark = Mark
            Invoke-Bot $bot "select $($k.weapon)" | Out-Null
            $ents = Invoke-Bot $bot 'entities' | Out-String
            Note "$($k.name) target" ($ents -split "`n" | Select-Object -First 6)
            # `attack` swings at the entity the bot faces by id (horizon-ctl attack --help).
            $target = (($ents | ConvertFrom-Json).entities | Where-Object { $_.type -eq $k.summon } | Select-Object -First 1).id
            Invoke-Bot $bot "look-at entity $target" | Out-Null
            Start-Sleep -Milliseconds 300
            $tags = Invoke-Console "data get entity @e[type=$($k.summon),limit=1,sort=nearest,x=$ax,y=$ay,z=$az] Tags"
            Note "$($k.name) tags" $tags
            if ($k.weapon -eq 'minecraft:bow') {
                # Aim at the body's middle and draw fully; shoot again (up to three times) until the
                # revenant is down, so the arrow really is the killing blow.
                for ($shot = 1; $shot -le 3; $shot++) {
                    Invoke-Bot $bot "look-at $($ax + 0.5) $($ay + 1.1) $($az - $far + 0.5)" | Out-Null
                    Invoke-Bot $bot 'mouse right down' | Out-Null
                    Start-Sleep -Milliseconds 1500
                    Invoke-Bot $bot 'mouse right up' | Out-Null
                    Start-Sleep -Milliseconds 1500
                    $left = ((Invoke-Bot $bot 'entities' | Out-String | ConvertFrom-Json).entities | Where-Object { $_.id -eq $target })
                    Note "$($k.name) shot $shot" "target left: $([bool]$left)"
                    if (-not $left) { break }
                }
            } else {
                Note "$($k.name) attack" (Invoke-Bot $bot "attack $target")
            }
            Start-Sleep -Milliseconds 600
            Save-Shot $bot $k.name
            Note "$($k.name) bar" (Bar)
            Note "$($k.name) log" (Get-LogSince $mark '(soulscraft: (death|adopt)|pct|path_of_craft_tree)')
            C 'kill @e[type=soulscraft:revenant,distance=..20,x=-500,y=101,z=500]'
            C 'kill @e[type=soulscraft:bog_slime,distance=..20,x=-500,y=101,z=500]'
        }
        C 'gamemode creative sc_a'
        C 'effect clear sc_a'
    }

    if ($Parts -contains 'boost') {
        # 6. A level-4 altar at night with the bot near: the boost's log lines with the switch on
        #    and off (soulscraft: boost …; `us` is our two injections' time per 200 ticks).
        C 'sc clear'
        C 'sc altar 4'
        C 'time set midnight'
        C 'difficulty normal'
        C "tp sc_a $($ax + 0.5) $($ay + 30) $($az + 0.5)"
        $mark = Mark
        Start-Sleep -Seconds 60
        Note 'boost on (60 s)' (Get-LogSince $mark 'soulscraft: (boost|adopt mob=.* fresh=false)')
        Save-Shot $bot 'boost_night_on'
        Invoke-Bot $bot 'select 9' | Out-Null
        C "tp sc_a $($ax + 0.5) $ay $($az + 3.5) 0 40"
        Start-Sleep -Seconds 1
        Invoke-Bot $bot "interact $ax $ay $($az + 6)" | Out-Null
        Start-Sleep -Seconds 2
        Save-Shot $bot 'menu_boost_line'
        Note 'menu with the boost line' (Invoke-Bot $bot 'screen')
        Invoke-Bot $bot 'key esc tap' | Out-Null
        C 'time set noon'
    }
} finally {
    C 'gamerule doDaylightCycle true'
    C 'gamemode creative sc_a'
    C 'effect clear sc_a'
    Stop-Bot $bot
    C 'sc clear'
}

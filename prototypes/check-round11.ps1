# Round 11's in-game checks (owner's follow-ups to iteration 3) with the bot sc_a on the joint server:
#   1. the 3D head items: every mob's head worn (third person, from the front and from the side),
#      the heads in the inventory (the hotbar and the open inventory), one in hand (first and third
#      person) and one on the ground;
#   2. the altar's menu with its long lines wrapped to the panel (a level-4 altar with sample
#      banners).
# Shots go to prototypes/evidence/round11/ (JPEG 960 wide; the PNGs stay in the scratchpad).
# Needs the joint server running offline with a client that has this build:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"   (and .\joint.ps1 stop, close after)
param([string[]]$Parts = @('heads', 'menu'))
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
$Out = Join-Path $PSScriptRoot 'evidence\round11'
$Raw = Join-Path $script:Scratch 'round11'
New-Item -ItemType Directory -Force $Out, $Raw | Out-Null
$Text = Join-Path $Out 'console.txt'
$Mobs = 'revenant', 'stalker', 'marksman', 'brute', 'shrike', 'lurker', 'bog_slime'
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
function C([string]$Line) { Invoke-Console $Line | Out-Null }

Set-Content $Text "round 11 checks, $(Get-Date -Format s)"
$bot = Start-Bot sc_a
try {
    C 'tick unfreeze'
    C 'sc arena'
    C 'sc clear'
    C 'gamemode creative sc_a'
    C 'clear sc_a'
    C 'time set noon'
    C 'weather clear'

    if ($Parts -contains 'heads') {
        # Worn: third person from the front (F5 twice), then from behind-left (F5 once more is first
        # person again, so turn the bot instead).
        C "tp sc_a $($ax + 0.5) $ay $($az + 4.5) 0 0"
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        # Each head from the front, the side and the back: the camera stays in front of the bot,
        # the bot turns (yaw 0, 90, 180).
        foreach ($m in $Mobs) {
            C "item replace entity sc_a armor.head with soulscraft:$($m)_head"
            foreach ($view in @(@('front', 0), @('side', 90), @('back', 180))) {
                C "tp sc_a $($ax + 0.5) $ay $($az + 4.5) $($view[1]) 0"
                Start-Sleep -Milliseconds 1200
                Save-Shot $bot "worn_$($m)_head_$($view[0])"
            }
        }
        C "tp sc_a $($ax + 0.5) $ay $($az + 4.5) 0 0"
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        C 'item replace entity sc_a armor.head with air'
        # The inventory: all seven heads in the hotbar, then the inventory screen.
        for ($i = 0; $i -lt $Mobs.Count; $i++) { C "item replace entity sc_a hotbar.$i with soulscraft:$($Mobs[$i])_head" }
        Invoke-Bot $bot 'select 4' | Out-Null
        C "tp sc_a $($ax + 0.5) $ay $($az + 4.5) 0 10"
        Start-Sleep -Seconds 1
        Save-Shot $bot 'heads_hotbar_in_hand'
        Note 'inventory' (Invoke-Bot $bot 'inventory' | Out-String)
        Invoke-Bot $bot 'open-inventory' | Out-Null
        Start-Sleep -Seconds 1
        Save-Shot $bot 'heads_inventory'
        Invoke-Bot $bot 'close' | Out-Null
        # In hand from the side (third person), and one dropped on the ground.
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        C "tp sc_a $($ax + 0.5) $ay $($az + 4.5) 300 0"
        Start-Sleep -Seconds 1
        Save-Shot $bot 'heads_in_hand_third_person'
        Invoke-Bot $bot 'key f5 tap' | Out-Null
        C "summon minecraft:item $($ax + 0.5) $ay $($az + 2.5) {Item:{id:`"soulscraft:revenant_head`",count:1},PickupDelay:32767,Age:-32768}"
        C "summon minecraft:item $($ax + 1.5) $ay $($az + 2.5) {Item:{id:`"soulscraft:lurker_head`",count:1},PickupDelay:32767,Age:-32768}"
        C "tp sc_a $($ax + 1) $ay $($az + 4.5) 180 35"
        Start-Sleep -Seconds 2
        Save-Shot $bot 'heads_on_the_ground'
        C 'kill @e[type=minecraft:item,x=-500,y=101,z=500,distance=..8]'
    }

    if ($Parts -contains 'menu') {
        C 'clear sc_a'
        C 'sc altar 4 banners'
        Start-Sleep -Seconds 2
        Invoke-Bot $bot 'select 9' | Out-Null
        C "tp sc_a $($ax + 0.5) $ay $($az + 3.5) 0 40"
        Start-Sleep -Seconds 1
        Invoke-Bot $bot "interact $ax $ay $($az + 6)" | Out-Null
        Start-Sleep -Seconds 2
        Save-Shot $bot 'menu_wrapped'
        Invoke-Bot $bot 'key esc tap' | Out-Null
    }
} finally {
    C 'gamemode creative sc_a'
    C 'clear sc_a'
    Stop-Bot $bot
    C 'sc clear'
}

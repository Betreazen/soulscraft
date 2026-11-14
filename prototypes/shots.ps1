# П2 and the fast-test kit: each mob (or one) spawned in front of the bot sc_a in the test arena,
# a screenshot in each phase of each of its attacks, then the attack timings in the server's log
# checked against data/mobs.json.
#   .\prototypes\shots.ps1                       every mob, every attack
#   .\prototypes\shots.ps1 -Mob revenant         one mob
#   .\prototypes\shots.ps1 -Mob revenant -Attack overhead
# Needs the joint server running in offline mode with a client build that has SoulsCraft:
#   .\joint.ps1 open; .\joint.ps1 serve -Owner "soulscraft agent"   (and close after)
# The world is frozen for the shots (/tick freeze) and stepped to each phase (/tick step), so a
# shot shows the tick the file name says. Shots go to prototypes\shots\<mob>\.
param([string]$Mob = '', [string]$Attack = '', [switch]$KeepBot)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib\bot.ps1')
. (Join-Path $PSScriptRoot 'lib\log.ps1')
$Repo = Split-Path $PSScriptRoot
$Out = Join-Path $PSScriptRoot 'shots'
$mobs = (Get-Content -Raw -Encoding utf8 (Join-Path $Repo 'data\mobs.json') | ConvertFrom-Json).mobs
if ($Mob) { $mobs = @($mobs | Where-Object id -eq $Mob) }
if (-not $mobs) { throw "No mob $Mob in data/mobs.json" }

# The arena's centre (tools.rs ARENA); /sc spawn from the console puts a mob 5 blocks south of it.
$cx = -499.5; $cy = 101; $cz = 500.5
$log = "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"

# `/tick step` returns at once and the ticks run after it (20 a second): wait them out, and a
# little more for the client to get the last positions and notes, before the next command.
function Step-World([int]$Ticks) {
    Invoke-Console "tick step $Ticks" | Out-Null
    Start-Sleep -Milliseconds ($Ticks * 50 + 1500)
}

$bot = Start-Bot sc_a
try {
    Invoke-Console 'sc arena' | Out-Null
    Invoke-Console 'sc debug on' | Out-Null
    Invoke-Console 'tick unfreeze' | Out-Null
    foreach ($m in $mobs) {
        $dir = Join-Path $Out $m.id
        New-Item -ItemType Directory -Force $dir | Out-Null
        Invoke-Console 'sc clear' | Out-Null
        # The bot 6 blocks north of the mob (9 for the brute and the shrike), facing it. The world is
        # frozen right after the spawn, so the mob stays where it stood: it moves only in the steps.
        $back = if ($m.id -in 'brute', 'shrike') { 9 } else { 6 }
        Invoke-Console 'tick unfreeze' | Out-Null
        Invoke-Console "tp sc_a $cx $cy $($cz + 5 - $back) 0 12" | Out-Null
        Invoke-Console "sc spawn $($m.id) 0 1" | Out-Null
        Invoke-Console 'tick freeze' | Out-Null
        Step-World 2
        Invoke-Bot $bot "screenshot $dir\idle.png" | Out-Null
        # Close up, from the front and from its right front, for the grip (П2): the bot 3 blocks off.
        Invoke-Console "tp sc_a $cx $cy $($cz + 2) 0 20" | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $bot "screenshot $dir\closeup_front.png" | Out-Null
        Invoke-Console "tp sc_a $($cx - 2.2) $cy $($cz + 2.8) -45 20" | Out-Null
        Start-Sleep -Seconds 2
        Invoke-Bot $bot "screenshot $dir\closeup_three_quarter.png" | Out-Null
        if ($m.id -eq 'revenant') {
            # The same sword in a vanilla zombie's hand (its own hand transform, render.rs:2412-2492)
            # two blocks to the revenant's left, both facing the bot.
            Invoke-Console "summon minecraft:zombie $($cx + 2) $cy $($cz + 5) {NoAI:1b,PersistenceRequired:1b,Rotation:[180f,0f],Tags:[`"soulscraft.grip`"],equipment:{mainhand:{id:`"soulscraft:rusty_sword`",count:1}}}" | Out-Null
            Invoke-Console "tp sc_a $($cx + 1) $cy $($cz + 1.5) 0 15" | Out-Null
            Step-World 1
            Invoke-Bot $bot "screenshot $dir\grip_vs_zombie.png" | Out-Null
            Invoke-Console 'kill @e[tag=soulscraft.grip]' | Out-Null
        }
        Invoke-Console "tp sc_a $cx $cy $($cz + 5 - $back) 0 12" | Out-Null
        Start-Sleep -Seconds 1
        $attacks = @($m.attacks | Where-Object { -not $_.follows })
        if ($Attack) { $attacks = @($attacks | Where-Object id -eq $Attack) }
        foreach ($a in $attacks) {
            $hit = $a.windup + $a.hold
            $stops = [ordered]@{ windup = [math]::Floor($a.windup / 2); hit = $hit; recover = $hit + $a.hit + 2 }
            if ($a.hold -gt 0) { $stops.Insert(1, 'hold', $a.windup + [math]::Floor($a.hold / 2)) }
            Invoke-Console "sc force $($a.id)" | Out-Null
            $at = -1
            foreach ($phase in $stops.Keys) {
                $step = $stops[$phase] - $at
                Step-World $step
                $at = $stops[$phase]
                Invoke-Bot $bot "screenshot $dir\$($a.id)_$($phase)_t$($at).png" | Out-Null
            }
            # On to the attack's end and a little past, still frozen between the attacks.
            $end = $hit + $a.hit + $a.recover
            Step-World ($end - $at + 6)
        }
    }
} finally {
    Invoke-Console 'tick unfreeze' | Out-Null
    Invoke-Console 'sc debug off' | Out-Null
    Invoke-Console 'sc clear' | Out-Null
    if (-not $KeepBot) { Stop-Bot $bot }
}

$rows = Read-SoulsLog $log
$timings = @(Test-AttackTimings $rows)
$timings | Format-Table mob, attack, got, ok -AutoSize
$bad = @($timings | Where-Object { -not $_.ok })
if ($bad.Count -gt 0) { throw "$($bad.Count) attack(s) off their data ticks" }
Write-Host "Shots in $Out; $($timings.Count) attacks on their data ticks."

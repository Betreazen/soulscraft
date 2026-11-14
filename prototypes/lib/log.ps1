# Reads SoulsCraft's log lines (mod/README.md, «Строки лога») from a server's latest.log into
# objects, and checks attack timings against data/mobs.json. Dot-source it:
#   . .\prototypes\lib\log.ps1
#   $rows = Read-SoulsLog "$env:LOCALAPPDATA\Horizon\joint-dev\server\logs\latest.log"
#   Test-AttackTimings $rows | Format-Table
$script:Repo = Split-Path (Split-Path $PSScriptRoot)

function Read-SoulsLog([string]$Path, [string]$Kind = '') {
    Select-String -Path $Path -Pattern 'soulscraft: (phase|strike|mode|adopt|death|blocked|wave|cost|selftest) (.*)$' |
        ForEach-Object {
            $m = $_.Matches[0]
            $row = [ordered]@{ kind = $m.Groups[1].Value }
            foreach ($pair in [regex]::Matches($m.Groups[2].Value, '(\w+)=(\S+)')) {
                $row[$pair.Groups[1].Value] = $pair.Groups[2].Value
            }
            [pscustomobject]$row
        } |
        Where-Object { $Kind -eq '' -or $_.kind -eq $Kind }
}

# Each attack run in the log: its phase ticks from its windup, against mobs.json's.
function Test-AttackTimings($Rows) {
    $mobs = (Get-Content -Raw -Encoding utf8 (Join-Path $script:Repo 'data\mobs.json') | ConvertFrom-Json).mobs
    $runs = @{}
    foreach ($r in ($Rows | Where-Object kind -eq 'phase')) {
        $key = "$($r.id)"
        if ($r.phase -eq 'windup') { $runs[$key] = [ordered]@{ mob = $r.mob; id = $r.id; attack = $r.attack; start = [int64]$r.tick; got = [ordered]@{} } }
        if ($runs.ContainsKey($key)) { $runs[$key].got[$r.phase] = [int64]$r.tick - $runs[$key].start }
        if ($r.phase -eq 'end' -and $runs.ContainsKey($key)) {
            $run = $runs[$key]
            $a = ($mobs | Where-Object id -eq $run.mob).attacks | Where-Object id -eq $run.attack
            $hit = $a.windup + $a.hold
            $want = [ordered]@{ hit = $hit; recover = $hit + $a.hit; end = $hit + $a.hit + $a.recover }
            if ($a.hold -gt 0) { $want.Insert(0, 'hold', $a.windup) }
            # A charge that hits no wall ends 10 ticks into its window (brain.rs CHARGE_OPEN_RECOVER).
            $openCharge = $a.recover_if -eq 'wall' -and $run.got['end'] -eq ($hit + $a.hit + 10)
            $bad = @($want.Keys | Where-Object { $run.got[$_] -ne $want[$_] -and -not ($_ -eq 'end' -and $openCharge) })
            [pscustomobject]@{
                mob = $run.mob; id = $run.id; attack = $run.attack; tick = $run.start
                got = ($run.got.Keys | ForEach-Object { "$_=$($run.got[$_])" }) -join ' '
                ok = $bad.Count -eq 0
            }
            $runs.Remove($key)
        }
    }
}

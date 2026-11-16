# Bots and the console on the live server tidewill (rustcraft-mc.games, pack-12 and later), for
# playtests on real terrain with the whole pack. Dot-source it after lib\bot.ps1:
#   . .\prototypes\lib\bot.ps1; . .\prototypes\lib\tidewill.ps1
#   Use-Tidewill -Instance "$env:APPDATA\horizon\instances\pack-bots"
#   $a = Start-Bot sc_a
#   Invoke-Remote @('cmd time set night', 'status')
#
# Rules (owner, 2026-11-16 03:12-03:15, via the RustCraft session):
#   - offline bots only (dev mode), never the owner's account; clients muted (Start-Bot does it);
#   - SoulsCraft plays in x -1000..0, z 0..1000 (the console's /sc commands work at the bench
#     arena -500,101,500 there), RustCraft in x 0..1000;
#   - the console goes through `sudo -u minecraft horizon-server --dir /srv/minecraft …`
#     (/etc/sudoers.d/tidewill-test, removed before the release with dev mode);
#   - at most 6 ssh connections in 30 s for both sessions together: every connection is noted in
#     a shared file and waits while 5 were made in the last 30 s; steps go in one connection.
$script:TidewillHost = 'tidewill'
$script:TidewillTool = 'sudo -n -u minecraft /home/minecraft/.local/bin/horizon-server --dir /srv/minecraft'
$script:TidewillLog = '/srv/minecraft/logs/latest.log'
$script:SshNotes ="$env:LOCALAPPDATA\Horizon\joint-dev\tidewill-ssh.log"
$script:SshWindow = 35
$script:SshMax = 4

function Use-Tidewill([Parameter(Mandatory)][string]$Instance) {
    if (-not (Test-Path $Instance)) { throw "No bot instance at $Instance" }
    $script:Instance = $Instance
    $script:InstanceName = Split-Path $Instance -Leaf
    $script:ServerAddress = 'rustcraft-mc.games:25565'
    $script:Remote = $true
}

# Waits until a connection fits in the shared budget, then notes it.
function Wait-SshBudget {
    New-Item -ItemType Directory -Force (Split-Path $script:SshNotes) | Out-Null
    while ($true) {
        $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        $recent = @()
        if (Test-Path $script:SshNotes) {
            $recent = @(Get-Content $script:SshNotes -ErrorAction SilentlyContinue |
                ForEach-Object { ($_ -split ' ')[0] } | Where-Object { $_ -match '^\d+$' -and ($now - [long]$_) -lt $script:SshWindow })
        }
        if ($recent.Count -lt $script:SshMax) { break }
        Start-Sleep -Seconds 3
    }
    Add-Content $script:SshNotes "$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) soulscraft"
}

# One ssh connection held for the whole run (a remote bash reading steps from its stdin), so a
# playtest costs one connection of the shared budget, not one per batch. Steps run in turn
# (cmd <line>, status --json, logs -n 200); one string array per step comes back, in order.
$script:Session = $null
$script:Seq = 0
function Open-Tidewill {
    Wait-SshBudget
    $psi = [Diagnostics.ProcessStartInfo]::new('ssh', "-o BatchMode=yes -o ConnectTimeout=15 -o ServerAliveInterval=20 $script:TidewillHost bash")
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $script:Session = [Diagnostics.Process]::Start($psi)
}
function Close-Tidewill {
    if ($script:Session -and -not $script:Session.HasExited) {
        $script:Session.StandardInput.Close()
        if (-not $script:Session.WaitForExit(10000)) { $script:Session.Kill() }
    }
    $script:Session = $null
}
function Invoke-Remote([string[]]$Steps) {
    if (-not $script:Session -or $script:Session.HasExited) { Open-Tidewill }
    $n = ++$script:Seq
    $lines = @()
    for ($i = 0; $i -lt $Steps.Count; $i++) {
        $words = $Steps[$i] -split ' ', 2
        $rest = if ($words.Count -gt 1) { " '" + ($words[1] -replace "'", "'\''") + "'" } else { '' }
        # cmd takes the console line as one argument; `grep N REGEX` gives the last N lines of
        # the game's log file (world-readable) that match, filtered on the server: `logs` stops at
        # 2000 lines, which a busy log outruns in a minute; other steps take their own words.
        $call = switch ($words[0]) {
            'cmd' { "$script:TidewillTool cmd$rest" }
            'grep' {
                $g = $words[1] -split ' ', 2
                "grep -E '$($g[1] -replace "'", "'\''")' $script:TidewillLog | tail -n $([int]$g[0])"
            }
            default { "$script:TidewillTool $($Steps[$i])" }
        }
        $lines += "echo '@@step $i'"
        # Braces: the redirections cover a whole pipeline (`logs` writes to stderr; without them
        # `tail` would read /dev/null).
        $lines += "{ $call ; } 2>&1 </dev/null"
    }
    $lines += "echo '@@end $n'"
    $script:Session.StandardInput.Write(($lines -join "`n") + "`n")
    $script:Session.StandardInput.Flush()
    $result = [Collections.Generic.List[string][]]::new($Steps.Count)
    for ($i = 0; $i -lt $Steps.Count; $i++) { $result[$i] = [Collections.Generic.List[string]]::new() }
    $at = -1
    while ($true) {
        $s = $script:Session.StandardOutput.ReadLine()
        if ($null -eq $s) {
            $err = $script:Session.StandardError.ReadToEnd()
            $script:Session = $null
            throw "tidewill: ssh session ended: $err"
        }
        if ($s -eq "@@end $n") { break }
        if ($s -match '^@@step (\d+)$') { $at = [int]$Matches[1]; continue }
        if ($at -ge 0) { $result[$at].Add($s) }
    }
    # The comma keeps one step's answer an array of lines: PowerShell would unroll it.
    , [string[][]]@($result | ForEach-Object { , $_.ToArray() })
}

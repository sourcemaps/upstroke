<#
windows-container-acceptance.ps1 -- bounded native Windows Container acceptance
capture (G7 M2, commission G7-M2-CONTAINER-ACCEPTANCE-20261109-1).

A native Windows coordinator -- this tree's test binary, built by the guest's
native MSVC Rust and native Git -- drives the Rancher Desktop Moby daemon through
the unmodified Docker CLI on npipe:////./pipe/docker_engine. Containers run in
the vendor's Linux environment, never natively on Windows. The vendor translates
native C:\ bind sources; the record keeps every inspect spelling as observed and
proves correspondence by behaviour, never by string equality.

Run in the task guest only (the machine name is checked), from a Git checkout of
the exact reviewed commit (the record reads HEAD and its tree from it), with
-OutDir and -WorkRoot naming paths that do not exist yet:

  powershell -NoProfile -ExecutionPolicy Bypass -File `
    .github\g7-platform-environment\windows-container-acceptance.ps1 `
    -OutDir C:\g7m2\out-<tag> -WorkRoot C:\g7m2\w-<tag> -ExpectedHead <reviewed sha>

It builds (`cargo test --all-features --lib --no-run`), runs the one ignored test
engine::tests::container_acceptance_record against -Image (present already;
never pulled), checks, decodes and hashes the framed record, and writes
SUMMARY.txt and MANIFEST.json. It changes no VM, network or Docker setting and
deletes nothing. Credential-like environment variables are removed from this
process by name before any child starts; no value is read or written. A child
that outlives its bound is ended with its process tree, through the handle this
script holds for it, and the step is recorded as a timeout (exit 124).
#>
param(
  [Parameter(Mandatory = $true)][string]$OutDir,
  [Parameter(Mandatory = $true)][string]$WorkRoot,
  [string]$ExpectedHead = '',
  [string]$Image = 'alpine:3.20',
  [string]$ExpectedMachine = 'G7PLAT-WIN1',
  [string]$Endpoint = 'npipe:////./pipe/docker_engine',
  [string]$ExpectClient = 'windows/amd64 29.9.3',
  [string]$ExpectServer = 'linux/amd64 29.9.2',
  [int]$BuildSeconds = 3600,
  [int]$RunSeconds = 1800
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$Test = 'engine::tests::container_acceptance_record'
$Marker = 'UPSTROKE-CONTAINER-ACCEPTANCE-RECORD'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ([Environment]::MachineName -ne $ExpectedMachine) { throw "wrong machine: $([Environment]::MachineName), expected $ExpectedMachine" }
foreach ($fresh in @($OutDir, $WorkRoot)) {
  if (Test-Path -LiteralPath $fresh) { throw "$fresh already exists; give a path this invocation creates" }
}
New-Item -ItemType Directory -Path $OutDir | Out-Null
$Tmp = Join-Path $WorkRoot 'tmp'
New-Item -ItemType Directory -Path $Tmp | Out-Null

$script:Seq = 0
$script:Failed = 0
$Checks = Join-Path $OutDir 'checks.tsv'
$Steps = Join-Path $OutDir 'steps.jsonl'
New-Item -ItemType File -Path $Checks | Out-Null
New-Item -ItemType File -Path $Steps | Out-Null

function Add-Check([string]$Name, [bool]$Pass, [string]$Detail) {
  $verdict = 'FAIL'
  if ($Pass) { $verdict = 'PASS' } else { $script:Failed++ }
  $clean = ($Detail -replace "[`t`r`n]", ' ')
  if ($clean.Length -gt 300) { $clean = $clean.Substring(0, 300) }
  $line = "$verdict`t$Name`t$clean"
  Add-Content -LiteralPath $Checks -Value $line
  Write-Output $line
}

function ConvertTo-ArgumentText([string]$Value) {
  if ($Value -ne '' -and $Value -notmatch '[\s"]') { return $Value }
  $text = '"'
  $slashes = 0
  foreach ($ch in $Value.ToCharArray()) {
    if ($ch -eq '\') { $slashes++; continue }
    if ($ch -eq '"') { $text += ('\' * (2 * $slashes + 1)) + '"' } else { $text += ('\' * $slashes) + $ch }
    $slashes = 0
  }
  return $text + ('\' * (2 * $slashes)) + '"'
}

# Invoke-Step NAME SECONDS PROGRAM ARGUMENTS -- bounded, stdout and stderr kept in files.
function Invoke-Step([string]$Name, [int]$Seconds, [string]$Program, [string[]]$Arguments) {
  $script:Seq++
  $base = Join-Path $OutDir ('{0:D2}-{1}' -f $script:Seq, $Name)
  $stdout = "$base.stdout.txt"
  $stderr = "$base.stderr.txt"
  $text = (@($Arguments) | ForEach-Object { ConvertTo-ArgumentText $_ }) -join ' '
  $started = Get-Date
  $process = Start-Process -FilePath $Program -ArgumentList $text -WorkingDirectory $Repo -NoNewWindow -PassThru `
    -RedirectStandardOutput $stdout -RedirectStandardError $stderr
  $null = $process.Handle
  if ($process.WaitForExit($Seconds * 1000)) {
    $process.WaitForExit()
    $rc = $process.ExitCode
  } else {
    & taskkill.exe /T /F /PID $process.Id | Out-File -LiteralPath "$base.timeout.txt"
    $process.WaitForExit()
    $rc = 124
  }
  $entry = [ordered]@{
    seq = $script:Seq; step = $Name; program = $Program; arguments = @($Arguments); rc = $rc
    seconds = [int]((Get-Date) - $started).TotalSeconds; stdout = (Split-Path -Leaf $stdout); stderr = (Split-Path -Leaf $stderr)
  }
  Add-Content -LiteralPath $Steps -Value ($entry | ConvertTo-Json -Compress -Depth 4)
  $script:LastStdout = $stdout
  return $rc
}

function Get-LastText { return [IO.File]::ReadAllText($script:LastStdout).Trim() }

# Names only: no value of a removed variable is read.
$removed = @()
foreach ($name in @(Get-ChildItem env: | ForEach-Object { $_.Name })) {
  $upper = $name.ToUpperInvariant()
  if ($upper -match '^(GIT_|GH_|GITHUB_|ANTHROPIC|OPENAI|OPENROUTER|CLAUDE|CODEX|GEMINI|GOOGLE_|AWS_|AZURE_|COPILOT|SSH_)' -or
      $upper -match 'TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL|KEY|AUTH') {
    Remove-Item -LiteralPath "env:$name"
    $removed += $name
  }
}
$env:DOCKER_HOST = $Endpoint
$env:TMP = $Tmp
$env:TEMP = $Tmp
$env:UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE = $Image
$env:UPSTROKE_TEST_TEMP_FOLDS_CASE = '1'
[ordered]@{
  removed_names = $removed
  child_names = @(Get-ChildItem env: | ForEach-Object { $_.Name } | Sort-Object)
  set = [ordered]@{ DOCKER_HOST = $Endpoint; TMP = $Tmp; TEMP = $Tmp; UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE = $Image; UPSTROKE_TEST_TEMP_FOLDS_CASE = '1' }
  statement = 'names only, besides the values this script set'
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $OutDir 'environment.json')

# Provenance: the native host, its toolchain and the tree.
$os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber, OSArchitecture
[ordered]@{
  machine = [Environment]::MachineName; user = [Environment]::UserName; os = $os
  powershell = $PSVersionTable.PSVersion.ToString(); repo = $Repo; work_root = $WorkRoot
  scripts = @(Get-FileHash -Algorithm SHA256 -LiteralPath $PSCommandPath | ForEach-Object { [ordered]@{ path = $_.Path; sha256 = $_.Hash.ToLowerInvariant() } })
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutDir 'host.json')
$rc = Invoke-Step 'git-version' 60 'git' @('--version'); Add-Check 'native_git' ($rc -eq 0) (Get-LastText)
$rc = Invoke-Step 'rustc-version' 60 'rustc' @('-vV'); Add-Check 'native_rustc' (($rc -eq 0) -and ((Get-LastText) -match 'host: x86_64-pc-windows-msvc')) (Get-LastText)
$rc = Invoke-Step 'cargo-version' 60 'cargo' @('-V'); Add-Check 'native_cargo' ($rc -eq 0) (Get-LastText)
$rc = Invoke-Step 'source-head' 60 'git' @('-C', $Repo, 'rev-parse', 'HEAD', 'HEAD^{tree}')
$head = ((Get-LastText) -split "`r?`n")[0]
Add-Check 'source_is_a_checkout' ($rc -eq 0) (Get-LastText)
if ($ExpectedHead -ne '') { Add-Check 'source_is_the_reviewed_commit' ($head -eq $ExpectedHead) "head=$head expected=$ExpectedHead" }
$rc = Invoke-Step 'source-status' 60 'git' @('-C', $Repo, 'status', '--porcelain', '--untracked-files=no')
Add-Check 'source_has_no_tracked_changes' (($rc -eq 0) -and ((Get-LastText) -eq '')) (Get-LastText)

# The endpoint the setup established, and the image, which is never pulled.
$rc = Invoke-Step 'docker-versions' 60 'docker' @('version', '--format', '{{.Client.Os}}/{{.Client.Arch}} {{.Client.Version}} | {{.Server.Os}}/{{.Server.Arch}} {{.Server.Version}}')
Add-Check 'client_windows_server_linux' (($rc -eq 0) -and ((Get-LastText) -eq "$ExpectClient | $ExpectServer")) (Get-LastText)
$rc = Invoke-Step 'docker-info' 60 'docker' @('info', '--format', 'Name={{.Name}} OSType={{.OSType}} OperatingSystem={{.OperatingSystem}} KernelVersion={{.KernelVersion}} Architecture={{.Architecture}}')
Add-Check 'daemon_is_linux' (($rc -eq 0) -and ((Get-LastText) -match ' OSType=linux ')) (Get-LastText)
$rc = Invoke-Step 'image-present' 60 'docker' @('image', 'inspect', $Image, '--format', '{{.Id}} {{json .RepoDigests}} {{.Os}}/{{.Architecture}}')
Add-Check 'image_present_without_pull' ($rc -eq 0) (Get-LastText)

$Captured = $false
if ($script:Failed -eq 0) {
  $rc = Invoke-Step 'cargo-build' $BuildSeconds 'cargo' @('test', '--all-features', '--lib', '--no-run')
  Add-Check 'test_binary_built' ($rc -eq 0) "rc=$rc"
}
if ($script:Failed -eq 0) {
  $rc = Invoke-Step 'acceptance-run' $RunSeconds 'cargo' @('test', '--all-features', '--lib', $Test, '--', '--ignored', '--exact', '--nocapture')
  $runOut = $script:LastStdout
  Add-Check 'acceptance_test_exit_zero' ($rc -eq 0) "rc=$rc"
  $capture = @(Get-Content -LiteralPath $runOut)
  Add-Check 'exactly_the_one_acceptance_test_passed' (($capture -ccontains "test $Test ... ok") -and ($capture -cmatch '^test result: ok\. 1 passed; 0 failed; 0 ignored; ')) 'pass line and result line'
  $frames = @($capture | Where-Object { $_ -cmatch "^$Marker " })
  $frames | Set-Content -LiteralPath (Join-Path $OutDir 'frame.txt')
  $f = @($frames | ForEach-Object { ,($_ -split ' ') })
  $ok = $f.Count -ge 3 -and $f[0].Count -eq 7 -and $f[-1].Count -eq 7 -and $f[0][1] -ceq 'v1' -and $f[0][2] -ceq 'BEGIN' -and $f[-1][1] -ceq 'v1' -and $f[-1][2] -ceq 'END' -and ($f[0][3..6] -join ' ') -ceq ($f[-1][3..6] -join ' ') -and $f[0][4] -ceq "$($f.Count - 2)"
  for ($k = 1; $ok -and $k -lt $f.Count - 1; $k++) { $ok = $f[$k].Count -eq 5 -and $f[$k][1] -ceq 'v1' -and $f[$k][2] -ceq $f[0][3] -and $f[$k][3] -ceq "$k" -and $f[$k][4].Length -eq $(if ($k -lt $f.Count - 2) { 192 } else { 2 * ([long]$f[0][5] - ([long]$f[0][4] - 1) * 96) }) }
  Add-Check 'record_frame_complete' $ok "lines=$($f.Count)"
  if ($ok) {
    $hex = -join ($f[1..($f.Count - 2)] | ForEach-Object { $_[4] })
    # A faster decoder where the runtime has one; the loop below decodes the same bytes.
    $null = [Reflection.Assembly]::LoadWithPartialName('System.Runtime.Remoting')
    $soap = 'System.Runtime.Remoting.Metadata.W3cXsd2001.SoapHexBinary' -as [type]
    if ($null -ne $soap) {
      $bytes = $soap::Parse($hex).Value
    } elseif ($null -ne [Convert].GetMethod('FromHexString', [type[]]@([string]))) {
      $bytes = [Convert]::FromHexString($hex)
    } else {
      $bytes = New-Object byte[] ($hex.Length / 2)
      for ($i = 0; $i -lt $bytes.Length; $i++) { $bytes[$i] = [Convert]::ToByte($hex.Substring(2 * $i, 2), 16) }
    }
    $record = Join-Path $OutDir 'record.json'
    [IO.File]::WriteAllBytes($record, $bytes)
    $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $record).Hash.ToLowerInvariant()
    Add-Check 'record_decodes_to_its_digest' (($got -ceq $f[0][6]) -and ("$($bytes.Length)" -ceq $f[0][5])) "sha256=$got want=$($f[0][6]) bytes=$($bytes.Length)"
    $Captured = $true
  }
  $rc = Invoke-Step 'leftover-labelled-containers' 60 'docker' @('ps', '-a', '--filter', 'label=upstroke.private_root', '--format', '{{.Names}} {{.Status}}')
  Add-Check 'no_labelled_container_left' (($rc -eq 0) -and ((Get-LastText) -eq '')) (Get-LastText)
}

$verdict = 'CAPTURE_NOT_ESTABLISHED'
if ($Captured -and $script:Failed -eq 0) { $verdict = 'CAPTURED' }
@(
  "verdict=$verdict failed_checks=$($script:Failed)"
  '--- checks (PASS/FAIL decide)'
  (Get-Content -LiteralPath $Checks)
  '--- not claimed here: Gate7 or M2 acceptance (root judges the decoded record), native Windows'
  '    container execution (containers run in the vendor Linux environment), kills, crashes or cleanup'
) | Set-Content -LiteralPath (Join-Path $OutDir 'SUMMARY.txt')
@(Get-ChildItem -LiteralPath $OutDir -File | Sort-Object Name | ForEach-Object {
  [ordered]@{ file = $_.Name; bytes = $_.Length; sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant() }
}) | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $OutDir 'MANIFEST.json')
Get-Content -LiteralPath (Join-Path $OutDir 'SUMMARY.txt')
if ($verdict -ne 'CAPTURED') { exit 1 }
exit 0

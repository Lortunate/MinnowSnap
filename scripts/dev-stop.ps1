$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$debugExe = Join-Path $repoRoot "target\debug\MinnowSnap.exe"

if (Test-Path -LiteralPath $debugExe) {
    & $debugExe shutdown
    exit $LASTEXITCODE
}

cargo run -- shutdown
exit $LASTEXITCODE

param(
    [string]$ExecuteMethod = "HeadlessCliRunner.CompileAndVerify"
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
$ProjectPath = Join-Path $RootDir "unity_client"

$UnityPath = $env:UNITY_PATH
if (-not $UnityPath) {
    if ($IsMacOS) {
        $UnityPath = "/Applications/Unity/Hub/Editor/6000.6.7f1/Unity.app/Contents/MacOS/Unity"
    } else {
        $Resolved = Get-ChildItem "C:\Program Files\Unity\Hub\Editor\*\Editor\Unity.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
        $UnityPath = if ($Resolved) { $Resolved.FullName } else { "Unity.exe" }
    }
}

$CacheDir = Join-Path $ProjectPath "Library\StevedoreCache"
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
$env:STEVEDORE_CACHE_FOLDER = $CacheDir
$env:XDG_CACHE_HOME = $CacheDir
$env:LOCALAPPDATA = $CacheDir

& "$UnityPath" -batchmode -nographics -projectPath "$ProjectPath" -executeMethod $ExecuteMethod -quit -logFile -
exit $LASTEXITCODE


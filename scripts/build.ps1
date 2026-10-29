param(
    [switch]$SkipTests,
    [switch]$UseCurrentToolchain,
    [string]$OutputDirectory = 'dist'
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    $taskIsolatedCargo = Join-Path $env:USERPROFILE '.cache\glass-todo-build\cargo\bin\cargo.exe'
    if (!$UseCurrentToolchain -and (Test-Path -LiteralPath $taskIsolatedCargo)) { . (Join-Path $PSScriptRoot 'dev-env.ps1') }
    if (!$SkipTests) { cargo test --locked; if ($LASTEXITCODE -ne 0) { throw 'Tests failed' } }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    if ([System.IO.Path]::IsPathRooted($OutputDirectory)) { $taskDist = [System.IO.Path]::GetFullPath($OutputDirectory) }
    else { $taskDist = [System.IO.Path]::GetFullPath((Join-Path $taskRoot $OutputDirectory)) }
    if ($taskDist -eq $taskRoot) { throw 'Choose a separate output directory' }
    New-Item -ItemType Directory -Force -Path $taskDist | Out-Null
    $taskSourceExe = Join-Path $taskRoot 'target\release\glass-todo.exe'
    $taskDistExe = Join-Path $taskDist 'GlassTodo.exe'
    $taskCopyExe = !(Test-Path -LiteralPath $taskDistExe)
    if (!$taskCopyExe) { $taskCopyExe = (Get-FileHash -LiteralPath $taskSourceExe).Hash -ne (Get-FileHash -LiteralPath $taskDistExe).Hash }
    if ($taskCopyExe) { Copy-Item -LiteralPath $taskSourceExe -Destination $taskDistExe -Force }
    $taskRootDocs = @('README.md','README.en.md','LICENSE','CHANGELOG.md','CONTRIBUTING.md','SECURITY.md')
    foreach ($taskDoc in $taskRootDocs) { Copy-Item -LiteralPath (Join-Path $taskRoot $taskDoc) -Destination (Join-Path $taskDist $taskDoc) -Force }
    foreach ($taskFolder in @('docs','examples','third-party')) {
        $taskFolderOut = Join-Path $taskDist $taskFolder
        New-Item -ItemType Directory -Force -Path $taskFolderOut | Out-Null
        Get-ChildItem -LiteralPath (Join-Path $taskRoot $taskFolder) | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $taskFolderOut -Recurse -Force
        }
    }
    $taskVersion = ((Select-String -LiteralPath (Join-Path $taskRoot 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value)
    $taskZip = Join-Path $taskDist "GlassTodo-$taskVersion-windows-x64.zip"
    $taskArchiveItems = @($taskDistExe) + @($taskRootDocs | ForEach-Object { Join-Path $taskDist $_ }) + @((Join-Path $taskDist 'docs'),(Join-Path $taskDist 'examples'),(Join-Path $taskDist 'third-party'))
    Compress-Archive -LiteralPath $taskArchiveItems -DestinationPath $taskZip -Force
    $taskDigest = (Get-FileHash -LiteralPath $taskZip -Algorithm SHA256).Hash.ToLowerInvariant()
    [System.IO.File]::WriteAllText("$taskZip.sha256", "$taskDigest  $([System.IO.Path]::GetFileName($taskZip))`n", [System.Text.Encoding]::ASCII)
    Get-Item -LiteralPath $taskZip,"$taskZip.sha256" | Select-Object Name,Length,FullName
} finally { Pop-Location }

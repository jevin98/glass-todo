param([string]$ToolchainRoot = (Join-Path $env:USERPROFILE '.cache\glass-todo-build'))
$taskCache = [System.IO.Path]::GetFullPath($ToolchainRoot)
if (!(Test-Path -LiteralPath (Join-Path $taskCache 'cargo\bin\cargo.exe')) -or !(Test-Path -LiteralPath (Join-Path $taskCache 'mingw\w64devkit\bin\gcc.exe'))) {
    throw 'Isolated GNU toolchain not found. Use -ToolchainRoot or an existing Developer PowerShell; see docs/development.md.'
}
$env:RUSTUP_HOME = Join-Path $taskCache 'rustup'
$env:CARGO_HOME = Join-Path $taskCache 'cargo'
$taskCompiler = Join-Path $taskCache 'mingw\w64devkit\bin'
$env:PATH = "$env:CARGO_HOME\bin;$taskCompiler;$env:PATH"
$env:CC = Join-Path $taskCompiler 'gcc.exe'
$env:AR = Join-Path $taskCompiler 'ar.exe'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $env:CC
$env:RUSTFLAGS = '-C link-self-contained=yes'

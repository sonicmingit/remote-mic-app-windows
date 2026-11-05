param(
    [Parameter(Mandatory = $true)][string]$Repository,
    [string]$PatchPath = (Join-Path $PSScriptRoot 'doubao-clipboard-dictation.patch'),
    [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'
$patch = (Resolve-Path -LiteralPath $PatchPath).Path
$root = (& git -C $Repository rev-parse --show-toplevel)
if ($LASTEXITCODE -ne 0) { throw '指定目录不是 Git 仓库。' }

# Repeat application is a no-op. Never use --reject (partial application), reset,
# or text replacement to force a newer upstream revision to accept this patch.
$previousErrorPreference = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
& git -C $root apply --reverse --check $patch 2>$null
$alreadyApplied = ($LASTEXITCODE -eq 0)
$ErrorActionPreference = $previousErrorPreference
if ($alreadyApplied) {
    Write-Host '剪贴板听写补丁已经应用，无需重复操作。'
    exit 0
}
& git -C $root apply --check $patch
if ($LASTEXITCODE -ne 0) { throw '补丁与当前版本冲突；没有修改文件。请按补丁说明迁移入口。' }
if ($CheckOnly) {
    Write-Host '补丁检查通过，未修改文件。'
    exit 0
}

$branch = (& git -C $root branch --show-current)
if (-not $branch -or $branch -eq 'main') { throw '请先在最新 origin/main 上创建功能分支，再应用补丁。' }
& git -C $root diff --quiet
if ($LASTEXITCODE -ne 0) { throw '请先保存已有的已跟踪文件修改。' }
& git -C $root diff --cached --quiet
if ($LASTEXITCODE -ne 0) { throw '请先保存暂存区中的已有修改。' }
& git -C $root apply $patch
if ($LASTEXITCODE -ne 0) { throw '补丁没有应用成功，请检查 Git 输出。' }
Write-Host '补丁已应用。请执行前端检查并构建本地测试包；已安装的程序不会自动改变。'

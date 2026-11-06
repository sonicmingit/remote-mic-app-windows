# 豆包剪贴板听写补丁

## 自动浮窗投送（用户确认接字后追加）

连接 → 豆包输入法 → 开启“浮窗听写后自动粘贴（测试）”。在目标输入框放好光标，
按住语音键后文字先进入原生浮窗；松开且豆包提交文字后，复制本轮全文，隐藏浮窗，
恢复原输入框并发送一次 Ctrl+V。重启后开关关闭；失败保留文字并可用“复制全部”。
此入口每次按住开始一段新文字。下方“打开剪贴板听写”仍为手动累积入口。

完整源码补丁现在也包含 `crates/sayall-windows/src/clipboard_probe*.rs`、
`clipboard_delivery_windows.rs`、BLE 会话通知、平台退出接线与 Tauri IPC；
不改基础语音按键和音频生命周期，不应再按“仅前端补丁”理解。
重新构建前必须完成 Rust/Windows 检查，未来上游重构这些接点时须处理冲突。
该补丁不是安装目录覆盖包。真实硬件和第三方输入框投送边界见专项验收记录。

## 用法

连接 → 语音输入设置 → 豆包输入法 → “打开剪贴板听写”。点窗口内的输入框，
确认 Windows 当前输入法为豆包，然后按住遥控器语音键说话。松开并等文字出现后，
窗口自动复制**整个输入框**；切回需要输入的软件按 Ctrl + V。可以连续说几段，
最后一起粘贴；开始新的内容前点“清空内容”。

- 普通直接输入路径保持原样；需要粘贴时主动打开听写窗口。
- 只复制豆包已经确认并写到此窗口的文字，不读取豆包进程、私有数据或其他应用全文。
  没有识别出来的语音无法凭空恢复；这个入口解决目标软件无法直接接收文字的问题。
- 自动复制也适用于在窗口里修改文字。保留换行、空格与表情。
- “清空内容”只清空窗口，保留剪贴板；“复制全部”用于重试或再次复制。
- 切走窗口会取消尚未提交的复制，文字保留；重新聚焦后可以继续说话或手动复制。
  已经提交给系统的 Clipboard API 操作无法撤销；测试覆盖迟到结果不更新新状态。
- 关闭窗口会丢弃窗口中的文字；剪贴板保留。文字不写配置、不写回眸、不写日志。
  豆包自身的识别、网络和麦克风设置仍按其正常工作方式处理。

## 更新后重新应用

这是**源码补丁**，不能直接覆盖已经安装的 EXE。需要在新版源码上应用后重新构建，
也不能保证任意未来版本无冲突。手动入口集中在 `src/clipboard-dictation/`、独立入口组件、
Vite 页面、英文词条与两份 capability；自动入口另含 Windows 原生浮窗、剪贴板与焦点恢复、
BLE 会话开始/结束通知及 Tauri IPC。基础语音按键与音频时序不变。

制作基线为 SayAll 1.8.3，`origin/main` 提交 `87cf66c4f1058489bbd0cad0f802e2aa893344f8`。
可重用补丁不携带中央 `TODO.md` / `ATTRIBUTION.md` 的追加，减少新版合并冲突；
当前开发提交保留了这两份记录。

交付目录包含 `doubao-clipboard-dictation.patch` 与 `apply-clipboard-dictation-patch.ps1`。
先从最新主线创建分支（已有此功能时不要重复应用）：

```powershell
git fetch origin main
git switch -c codex/clipboard-dictation-update origin/main
powershell -File <补丁目录>/apply-clipboard-dictation-patch.ps1 -Repository . -CheckOnly
powershell -File <补丁目录>/apply-clipboard-dictation-patch.ps1 -Repository .
pnpm install --frozen-lockfile
pnpm test
pnpm build
```

也可直接用 `git apply --check <补丁文件>`，通过后 `git apply <补丁文件>`。
脚本拒绝在 main / detached HEAD 或已有 tracked 修改时写入，重复应用是空操作，
冲突时完整退出，不强制覆盖、不生成部分应用的 `.rej` 文件。

如上游重排连接页，须把含自动开关的 `ClipboardDictationLauncher` 接回豆包工具面板；
保留 Vite 的 `clipboard-dictation.html` 入口、capability、原生模块与会话/退出接线。
不要把未知冲突当作通过。
前端检查后，具备 Rust、MSVC 与仓库安装器依赖的 Windows 主机运行：

```powershell
powershell -File scripts/ci-preflight.ps1
powershell -File scripts/build-local-test-package.ps1
```

按 [WindowsClipboardDictation.md](../Testing/WindowsClipboardDictation.md) 完成安装与真机验收后再提交。
本地安装包不自动发布。此补丁已在当前分支应用，不用再在这里运行一次。

## 2026-11-05 验证边界

- `passed`：`pnpm test` 584 passed / 10 skipped，`pnpm build` 类型检查和生产构建；专项测试覆盖组合输入、整段复制、
  连续编辑、慢写入串行化、复制失败与重试、关闭/失焦/清空、窗口重用/失败/超时、隐私与英文。
- `passed`：Windows PowerShell 5.1 中，干净基线的补丁预检、应用、重复应用全部通过，
  应用后 17 个文件与源文件逐字节一致（仅归一化 Git 的 CRLF/LF）。脚本使用 UTF-8 BOM，
  避免旧 PowerShell 将中文提示按本地代码页误读。
- `passed`：Edge 中生产构建页面的多行与表情复制，页面显示成功且浏览器会话剪贴板逐字读回一致。
- `deferred`：Windows 系统剪贴板独立粘贴。PowerShell 跨通道读取为空，
  不能把浏览器会话的读回扩大为系统剪贴板通过；随后原生 UI 自动化因无法可靠核对浏览器 URL 被工具停止。
- 后续本地出包（同日，用户明确要求生成安装包）：已补齐独立 Rust 1.99.1、MSVC
  14.44.35207 与 Windows SDK 10.0.26100.0，工具目录不进入安装包。
- `passed`：`cargo fmt --all -- --check`、`cargo test --workspace --locked`
  （780 passed / 17 ignored）、`cargo check --workspace --locked`、
  `cargo check -p sayall-windows-app --features runtime-simulation --locked`。
  首轮测试中预设应用候选路径用例失败，已复现为本机 `TEMP` / `TMP` 不一致；
  在测试进程中统一两者后全量通过，没有修改产品代码或用户的永久环境变量。
- `passed`：`scripts/build-local-test-package.ps1`、`scripts/verify-windows-bundle.ps1`、
  23/23 前端资源嵌入（含 `clipboard-dictation.html`）、源修订号内嵌读回、NSIS 归档完整性。
  安装器版本 1.8.3，x64，来源提交 `ea44a10e135d01189b598290b6dec568ed983077`，
  SHA-256 `6d0271852b560faff52f2e6a2d9c4078d3bd11bd59185bce841d8a88aa2e1415`。
- 包是公开源码构建，不含会员、常用语和键位方案私有组件；Windows Authenticode 未签名，
  updater `.sig` 使用本机测试密钥，不可当作官方更新资产发布。
- `deferred`：安装/升级/启动、真实豆包输入法与 RC001 / RC003 分别的冷态首用、
  连续语音、断连和睡眠恢复。当前主机正在使用正式安装版，未自动覆盖或卸载，
  CI 生命周期脚本所需的“无已有安装”条件不成立。安装后应按验收手册独立确认系统粘贴结果。

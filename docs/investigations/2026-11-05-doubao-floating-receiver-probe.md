# 豆包输入法浮窗听写与自动投送：测试记录

## 最新状态：复制并回到原输入框

用户已确认测试包的浮窗可出现且可接收豆包文字，并要求补齐松键后的整段投送。
实现新增：释放 + IME_END + 非空已提交文字后，复制全文并读回 → 隐藏浮窗 → 恢复按键开始时
保存的原窗口及其焦点 → 校验身份、焦点、剪贴板和修饰键 → 仅发送一次 Ctrl+V。
开关为“浮窗听写后自动粘贴（测试）”，重启关闭，原手动听写入口保留。

- `observed`：本机诊断日志最近 6 次非空接字均在松键后才出现文字，首个非空结果相对松键
  晚 659–817 ms，且均有 IME_END。未读取或归档正文；型号未由用户分别确认。
  因此等待真实组合提交事件，不把 0.66–0.82 秒写成固定 ASR 等待常量。
- 以代次和文字修订号合并队列中的通知，旧轮次、重复结束或迟到输入不能再投送。
  剪贴板占用和仍按住修饰键可轮询重试；30 秒仅用于失败收尾，不用于判定识别完成。
- 用户中途切窗、目标失效、焦点读回不符、正文在复制期间变化、新剪贴板内容、输入提交失败：
  停止自动粘贴并保留文字。复制成功后才隐藏；隐藏后的失败以不激活方式保留浮窗。
  “复制全部”仅手动复制，不重试自动粘贴，避免重复插入。
- Unicode 剪贴板在分配并填充后才清空；写入期间精确读回自己的数据。实测关闭剪贴板会
  推进序号且所有者仍匹配，所以变更监测基线取写入关闭完成后；粘贴前仍检查该基线与所有者。
  原生输入提交 accepted 单独记录，不能写作目标应用插入通过。
- 公开 API：GetForegroundWindow/GetGUIThreadInfo 保存原焦点；验证句柄的进程/线程身份与
  父子关系；SetForegroundWindow/SetFocus 与成对 AttachThreadInput 恢复，分离失败后台重试。
  不读取第三方输入框正文、配置或内存，不改原语音/音频 DOWN/UP 与结束时序。

本轮自动化、出包和完整真机验收结果追加在下方；豆包最终结果的完整性与第三方自动粘贴仍需实测。

### 本轮原生实验与环境阻塞

- `passed`（局部实验）：合成 Unicode 文本在写锁期间精确读回；关闭后重新打开剪贴板，
  非空格式和完整 UTF-16（换行及表情）再次逐字读回；只读打开/关闭不改变写入完成后的序号。
  未读取此前的用户剪贴板，也未操作可见窗口或发送全局键。
- `failed`（完整隐藏控件用例）：WM_PASTE 未得到文字，之后的重试在打开剪贴板时持续被拒绝。
  增加公开 GetOpenClipboardWindow / GetWindowThreadProcessId 诊断确认，当前锁由 GameViewer
  持有；3 秒内重试全部 busy。未关闭、终止或修改该应用。不能将这个用例记作 passed。
  完整隐藏控件粘贴暂为 `deferred`，需剪贴板正常可用后重跑上面的独立用例。
- 状态机与投送顺序测试覆盖复制前不隐藏、剪贴板 busy 可重试、目标/焦点/正文变化、恢复失败、
  新剪贴板变化、发送失败不重复、旧代次/重复/取消/空结果与释放前后两种提交顺序。
  新状态测试先运行得到 4 个预期失败，再实现；前端新文案先红再绿。

### 2026-11-06：环境恢复后的复验

- `passed`：公开 API 读回显示剪贴板锁已释放后，原样重跑
  `cargo test -p sayall-windows --lib clipboard_probe::native::delivery::tests::native_unicode_clipboard_readback_and_hidden_edit_paste --locked -- --ignored --exact`，
  1 passed，约 0.05 秒。完整 Unicode 文字在真实系统剪贴板及自有隐藏 EDIT 的 WM_PASTE 后
  逐字一致，并验证写入后的剪贴板变更能使旧序号失效。没有可见窗口、前台切换或全局按键。
  上面的失败实验保留作环境记录，本项不再 deferred。
- `passed`：源 `e7430010924d82f2f017050ceacfa95009c97c07` 的 `scripts/ci-preflight.ps1`
  全部 7 步：前端 586 passed / 10 skipped 与生产构建，Rust 794 passed / 18 ignored，
  fmt、workspace check、runtime-simulation feature check；仅测试进程统一 TMP=TEMP。
- `passed`：完整功能源码补丁在基线 `87cf66c4f1058489bbd0cad0f802e2aa893344f8` 的
  独立 Git 索引中预检、应用、与源文件逐字比对、反向预检；不修改当前工作区，也不含中央
  TODO/ATTRIBUTION 的追加。未来上游有冲突时必须处理冲突并重新构建，不能覆盖安装目录。
- `deferred`：完整豆包识别 → 原输入框 Ctrl+V 的可见结果、RC001/RC003 分别的冷态与连续会话、
  安装升级启动、断连/睡眠恢复。本轮不恢复此前被用户停止的界面自动化，不自动覆盖正在使用的安装版。

### 自动投送本地测试包

- 源 `e7430010924d82f2f017050ceacfa95009c97c07`，版本 1.8.3 / Windows x64。
  本地交付目录 `artifacts/local-test/2026-11-06-doubao-auto-paste/`，安装器
  `SayAll-1.8.3-doubao-auto-paste-x64-setup.exe`，13,135,609 字节。
  SHA-256：`147832ecd89b7aef4b8c85e3141a6fc53542ab0c007b137111b63a504b471dba`。
- `passed`：本地 NSIS 构建，bundle 载荷与 GUI 子系统检查，23/23 前端资源键嵌入，
  7z 归档完整性，安装器提取出的主程序源修订号与自动投送行为标记 5/5。
  提取程序与裸构建文件仅有 Tauri 的 `__TAURI_BUNDLE_TYPE_VAR_UNK` → `NSS` 三个字节差异；
  按锁定 `tauri-utils 2.9.3` 的 `src/platform.rs` 核实为 NSIS 标记，归一化后其余字节完全一致。
- 安装器仍为公开源码未签名本地测试包，不含会员、常用语、键位方案私有组件；updater 签名
  仅用本机测试密钥。未自动安装、覆盖、上传或发布。当前安装版在用，实际替换应先正常退出。
- 同目录提供 README、机器可读构建记录、校验清单与可重复源码补丁；补丁 SHA-256：
  `6e25ffe2422b744128dea47ef3995c481fec1b4324b0483b45a23c40d02e2e82`。
  此补丁对应上述代码提交的快照；更新后的实验/出包记录由本节与交付 README 补充。

## 用户目标与本轮范围

用户期望：在原目标输入框按住遥控器语音键 → 自动弹出接字浮窗 → 豆包将完整文字写入
浮窗 → 松开并识别完成后，复制本轮文字、回到原输入框、Ctrl+V 一次。原手动打开窗口
并手动粘贴的实现不满足这个目标。

对照 VoiceStick 后，用户明确选择继续用豆包输入法，先验证自动浮窗接字，不切换 API。
所以首轮代码只增加默认关闭、不持久化的“自动浮窗接字（测试）”，当时**没有自动粘贴**。
这不是上述完整功能。最初检查点未出新安装器，后续测试包记录见下方。

## 参考结论

来源：`sonicmingit/voicestick`，提交 `479bc330d99f47b47635694adfe83183ff4f7c6f`。

- `desktop/windows/src/voice_stick_coordinator.cc` 的 `FinishWithFinalText` / `CompletePendingPaste`：
  等 API 最终结果后执行一次投送，重复结果不能重复粘贴。
- `desktop/windows/src/input_injector_win.cc`：Unicode 剪贴板、Ctrl+V、外部焦点变化检查，
  不把 API 返回的 accepted 当成目标软件实际收到了文字。
- `desktop/windows/src/ime_voice_session.h` 和 `docs/豆包输入法使用说明.md`：
  输入法路径的音频/按键结束不是识别结束，该路径由豆包直接写目标输入框。
- `desktop/windows-s3/src-tauri/src/speech_overlay.rs` 的不激活浮窗是 API 字幕展示，
  不能直接作为接收豆包输入法文字的输入控件。

这里只参考行为和边界，没有复制 VoiceStick 实现，也没有读取其配置或凭据；参考仓库
原有的 IDE 修改保持原样。

## 原型实现

- `clipboard_probe.rs`：代次、释放、迟到输入和取消的状态；松开仍接收迟到文字。
- `clipboard_probe_windows.rs`：独立 Windows GUI 线程与原生 EDIT，语音线程只投递消息；
  默认关闭，切换到其他输入工具、断连/睡眠/中止/退出时收尾。每次按住重建 EDIT 与输入
  上下文，消息带代次，避免旧输入控件的通知污染新轮次。
- `ble.rs`：豆包 StreamStarted / StreamStopped 通知；不修改基础语音键时序、快捷键、
  音频或 Helper。浮窗在报告层硬件键之后才到位的风险必须实测，不能靠推断排除。
- 界面新增实验开关与 IPC；开关初始化失败回退，重启默认关闭。
- 日志仅含阶段、代次、字数、组合状态、焦点布尔值和耗时，不含听写正文。

## 已执行与未完成

- `passed`：新增状态测试先红后绿（初始 2 failed / 2 passed，随后 4 passed）；
  前端全量 586 passed / 10 skipped、`pnpm build`；独立原生窗口例程编译。
- `failed`：原生例程首次直接 `SetForegroundWindow` 的读回失败。进一步观测为
  `foreground_matches=false editor_matches=true`，不能误报自动聚焦成功。
- `observed`：原生例程看到了 Windows 输入法组合开始/结束与非零文字更新；
  **没有确认是一次完整的豆包语音识别**，不能写为语音验收通过。
- 已加入仓库既有的公开 `AttachThreadInput` 成对附加/分离尝试，**不注入 Alt 解锁**；
  分离失败保留状态，后续周期和退出重试。修改后的例程编译 passed，但自动聚焦行为未复验。
- 原生复验被用户物理 Esc 停止，此后不再执行界面操作。
- 后续按用户要求生成接字验证包，**没有恢复界面自动化，也没有自动安装或覆盖正式版**。
- `passed`：源 `1f017b9e237e70cc62d5c82128c276bcfdf66b02` 的
  `cargo fmt --all -- --check`、`cargo test --workspace --locked`（784 passed / 17 ignored，
  仅测试进程中令 TMP=TEMP）、`cargo check --workspace --locked`、
  `cargo check -p sayall-windows-app --features runtime-simulation --locked`。
  前端复用上一轮未变更代码的 586 passed / 10 skipped 与 `pnpm build` 结果。
- `passed`：`scripts/build-local-test-package.ps1`、`scripts/verify-windows-bundle.ps1`、
  23/23 前端资源嵌入、构建修订号和实验 IPC 标记读回、7z 归档完整性。
  Windows x64 / 1.8.3 安装器 SHA-256：
  `367fe03736cc9f7f591a7e04e4867a56f639d5c7d1906316a882848c8284d2c9`。
- 包为未签名的公开源码本地测试版，不含会员、常用语和键位方案的私有组件；updater
  签名仅使用本机测试密钥，不是官方发布资产。实际安装会替换同身份版本，不提供并装隔离。
- `deferred`：安装/升级/启动、自动聚焦与真实豆包、RC001/RC003、冷态首用、首尾完整性、
  连续会话、断连/睡眠恢复。自动复制和 Ctrl+V 投送尚未实现，不能将接字测试包作为完整功能交付。

## 首轮遗留项（历史检查点）

1. 只在用户恢复界面测试后运行原生例程，先验证焦点读回；不能只凭窗口可见就认定获得焦点。
2. 用真实豆包和遥控器确认整句首尾、冷态首按；核对浮窗聚焦时右 Alt 是否已经按下，以及
   松开、IME 提交和最终文本更新的顺序。WM_IME_ENDCOMPOSITION 不是未经验证的 ASR 最终结果接口。
3. 接字确认前，保持自动粘贴关闭。后续投送必须保存原目标和焦点、复制后读回、目标重验、
   Ctrl+V 一次；用户切窗、目标失效或重复/迟到结果时保留文字并取消自动投送。
4. 再完成自动化、真实验证、本地安装包与可重复补丁；不自动覆盖正式安装版或发布。

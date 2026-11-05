# 豆包输入法自动浮窗接字：未完成检查点

## 用户目标与本轮范围

用户期望：在原目标输入框按住遥控器语音键 → 自动弹出接字浮窗 → 豆包将完整文字写入
浮窗 → 松开并识别完成后，复制本轮文字、回到原输入框、Ctrl+V 一次。原手动打开窗口
并手动粘贴的实现不满足这个目标。

对照 VoiceStick 后，用户明确选择继续用豆包输入法，先验证自动浮窗接字，不切换 API。
所以本轮代码只增加默认关闭、不持久化的“自动浮窗接字（测试）”，**没有自动粘贴**。
这不是上述完整功能，也没有生成替代上次安装包的新安装器。

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
- `deferred`：最新代码的 Rust 全量测试、默认/runtime-simulation 编译检查、完整安装器，
  以及真实豆包、RC001/RC003、冷态首用、首尾完整性、连续会话、断连/睡眠恢复。

## 恢复工作时先做

1. 只在用户恢复界面测试后运行原生例程，先验证焦点读回；不能只凭窗口可见就认定获得焦点。
2. 用真实豆包和遥控器确认整句首尾、冷态首按；核对浮窗聚焦时右 Alt 是否已经按下，以及
   松开、IME 提交和最终文本更新的顺序。WM_IME_ENDCOMPOSITION 不是未经验证的 ASR 最终结果接口。
3. 接字确认前，保持自动粘贴关闭。后续投送必须保存原目标和焦点、复制后读回、目标重验、
   Ctrl+V 一次；用户切窗、目标失效或重复/迟到结果时保留文字并取消自动投送。
4. 再完成自动化、真实验证、本地安装包与可重复补丁；不自动覆盖正式安装版或发布。

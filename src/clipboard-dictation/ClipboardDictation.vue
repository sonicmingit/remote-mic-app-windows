<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "../lib/i18n";
import { reportFrontendEvent } from "../lib/frontend-diagnostics";

// Isolated receiving surface: Doubao writes into our editor through its normal IME path.
// No third-party text inspection, audio recording, persistence, or timing assumptions.
const editor = ref<HTMLTextAreaElement>();
const text = ref("");
const composing = ref(false);
const status = ref<"empty" | "ready" | "copying" | "copied" | "failed" | "composing">("empty");
const canCopy = computed(() => text.value.trim().length > 0 && !composing.value);
let revision = 0;
let operation = 0;
let pending: { text: string; revision: number } | null = null;
let writing: { text: string; revision: number } | null = null;
let copiedText: string | null = null;
let focused = true;
let disposed = false;

function log(phase: string, result: "passed" | "failed" | "unknown", reason: string, id: number) {
  reportFrontendEvent({ event: "clipboard_dictation_copy", phase, result, reason, detail: `copy_${id}` });
}

// One clipboard writer at a time; coalesce pending edits to the newest whole buffer.
// A Clipboard API operation already submitted to Windows cannot be cancelled.
async function pump(): Promise<void> {
  if (writing !== null || disposed || !focused || !pending) return;
  const copy = pending;
  pending = null;
  writing = copy;
  const id = ++operation;
  log("requested", "unknown", "committed_text", id);
  try {
    if (!navigator.clipboard?.writeText) throw new Error("clipboard_unavailable");
    await navigator.clipboard.writeText(copy.text);
    if (!disposed && copy.revision === revision) {
      copiedText = copy.text;
      status.value = "copied";
      log("completed", "passed", "write_resolved", id);
    } else {
      log("completed", "unknown", "superseded_or_cancelled", id);
    }
  } catch {
    if (!disposed && copy.revision === revision) {
      status.value = "failed";
      log("completed", "failed", "clipboard_write_failed", id);
    } else {
      log("completed", "unknown", "superseded_or_cancelled", id);
    }
  } finally {
    writing = null;
    void pump();
  }
}

function queueCopy(force = false): void {
  if (disposed || !focused || composing.value) return;
  if (!text.value.trim()) { invalidate(); return; }
  if (!force && (pending?.text === text.value || (writing?.text === text.value && writing.revision === revision && !pending))) return;
  if (!force && copiedText === text.value && status.value === "copied") return;
  pending = { text: text.value, revision: ++revision };
  status.value = "copying";
  queueMicrotask(() => { void pump(); });
}

function invalidate(): void {
  revision += 1;
  pending = null;
  copiedText = null;
  status.value = text.value ? "ready" : "empty";
}

function input(event: Event): void {
  text.value = (event.target as HTMLTextAreaElement).value;
  if (composing.value || (event as InputEvent).isComposing) {
    invalidate();
    status.value = "composing";
    return;
  }
  queueCopy();
}

function compositionStart(): void {
  composing.value = true;
  invalidate();
  status.value = "composing";
}

function compositionEnd(): void {
  composing.value = false;
  // Some IMEs send the final input event after compositionend. Read the final DOM
  // in a microtask; the matching input event is deduplicated by queueCopy.
  queueMicrotask(() => {
    if (disposed || composing.value || !focused) return;
    text.value = editor.value?.value ?? text.value;
    queueCopy();
  });
}

function copyAll(): void { queueCopy(true); editor.value?.focus(); }
function clear(): void {
  text.value = "";
  if (editor.value) editor.value.value = "";
  invalidate();
  editor.value?.focus();
}
function blur(): void { focused = false; invalidate(); }
function focus(): void { focused = true; editor.value?.focus(); }
function stop(): void { disposed = true; invalidate(); }

onMounted(() => {
  editor.value?.focus();
  window.addEventListener("blur", blur);
  window.addEventListener("focus", focus);
  window.addEventListener("pagehide", stop);
});
onUnmounted(() => {
  stop();
  window.removeEventListener("blur", blur);
  window.removeEventListener("focus", focus);
  window.removeEventListener("pagehide", stop);
});
</script>

<template>
  <main class="clipboard-dictation">
    <h1>{{ t("剪贴板听写") }}</h1>
    <p>{{ t("点下面的输入框，按住遥控器语音键说话，松开后等待文字出现。") }}</p>
    <p class="muted">{{ t("请使用豆包输入法。文字确认后自动复制整个输入框，可分几次说完，再到目标软件粘贴。") }}</p>
    <label for="dictation-text">{{ t("听写内容") }}</label>
    <textarea
      id="dictation-text"
      ref="editor"
      :value="text"
      :placeholder="t('识别出的文字会显示在这里')"
      spellcheck="false"
      @input="input"
      @compositionstart="compositionStart"
      @compositionend="compositionEnd"
    ></textarea>
    <div class="dictation-actions">
      <button class="primary-button" data-action="copy" type="button" :disabled="!canCopy" @click="copyAll">{{ t("复制全部") }}</button>
      <button class="secondary-button" data-action="clear" type="button" :disabled="!text || composing" @click="clear">{{ t("清空内容") }}</button>
    </div>
    <p role="status" :class="{ 'dictation-error': status === 'failed', 'dictation-success': status === 'copied' }">
      <template v-if="status === 'copied'">{{ t("已复制全部文字，切到目标软件按 Ctrl + V 粘贴。") }}</template>
      <template v-else-if="status === 'failed'">{{ t("复制失败，文字已保留。请点“复制全部”重试，或选中文字后按 Ctrl + C。") }}</template>
      <template v-else-if="status === 'copying'">{{ t("正在复制全部文字…") }}</template>
      <template v-else-if="status === 'composing'">{{ t("正在输入，确认文字后会自动复制。") }}</template>
      <template v-else-if="status === 'ready'">{{ t("文字已保留，点“复制全部”后再粘贴。") }}</template>
      <template v-else>{{ t("等待听写。清空内容不会清空剪贴板。") }}</template>
    </p>
    <p class="muted dictation-privacy">{{ t("关闭窗口会清除这里的文字，剪贴板内容保留。") }}</p>
  </main>
</template>

<style scoped>
.clipboard-dictation { display: flex; flex-direction: column; gap: 12px; padding: 24px; min-height: 100vh; box-sizing: border-box; }
h1 { margin: 0; font-size: 24px; }
p { margin: 0; line-height: 1.6; }
label { font-weight: 600; }
textarea { flex: 1; min-height: 160px; width: 100%; resize: vertical; box-sizing: border-box; padding: 12px; font: inherit; line-height: 1.6; color: var(--text-primary); background: var(--card); border: 1px solid var(--border-strong); border-radius: 10px; }
textarea:focus { outline: 2px solid var(--accent); outline-offset: 2px; }
.dictation-actions { display: flex; flex-wrap: wrap; gap: 10px; }
.dictation-error { color: var(--error-text); }
.dictation-success { color: var(--success-text); }
.dictation-privacy { font-size: 13px; }
</style>

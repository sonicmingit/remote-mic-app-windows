<script setup lang="ts">
import { ref } from "vue";
import { t } from "../lib/i18n";
import { openClipboardDictation } from "../clipboard-dictation/open";
import { reportFrontendEvent } from "../lib/frontend-diagnostics";

const busy = ref(false);
const error = ref(false);
async function open() {
  busy.value = true;
  error.value = false;
  reportFrontendEvent({ event: "clipboard_dictation_open", phase: "requested", result: "unknown", reason: "user_click" });
  try {
    await openClipboardDictation();
    reportFrontendEvent({ event: "clipboard_dictation_open", phase: "completed", result: "passed", reason: "window_created_or_focused" });
  } catch {
    error.value = true;
    reportFrontendEvent({ event: "clipboard_dictation_open", phase: "completed", result: "failed", reason: "window_unavailable" });
  } finally { busy.value = false; }
}
</script>

<template>
  <div class="clipboard-dictation-launcher">
    <button type="button" class="secondary-button" :disabled="busy" @click="open">{{ t("打开剪贴板听写") }}</button>
    <p class="tiny muted">{{ t("无法直接输入时，先在听写窗口说话，文字会自动复制，之后可粘贴到其他软件。") }}</p>
    <p v-if="error" role="alert">{{ t("听写窗口没有打开，请重试。") }}</p>
  </div>
</template>

<style scoped>
.clipboard-dictation-launcher { margin-top: 12px; }
p { margin: 8px 0 0; }
[role="alert"] { color: var(--error-text); }
</style>

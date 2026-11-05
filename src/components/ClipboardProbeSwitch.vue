<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../lib/bridge";
import { effectiveLanguage } from "../lib/language-state";
import { t } from "../lib/i18n";

const checked = ref(false);
const loading = ref(true);
const busy = ref(false);
const error = ref(false);
const supported = isTauriRuntime();
let disposed = false;
let revision = 0;
let timer: ReturnType<typeof setInterval> | undefined;
async function refresh() {
  if (!supported || busy.value) return;
  const requestedRevision = revision;
  try { const value = await invoke<boolean>("get_clipboard_probe_enabled"); if (!disposed && !busy.value && requestedRevision === revision) checked.value = value; }
  catch { /* Initial load may precede native host availability. Keep the experiment off. */ }
}
async function change(event: Event) {
  const desired = (event.target as HTMLInputElement).checked;
  const previous = checked.value;
  revision += 1;
  checked.value = desired;
  busy.value = true;
  error.value = false;
  try { checked.value = await invoke<boolean>("set_clipboard_probe_enabled", { enabled: desired, english: effectiveLanguage.value === "en" }); }
  catch { checked.value = previous; error.value = true; }
  finally { busy.value = false; }
}
onMounted(async () => {
  await refresh();
  if (disposed) return;
  loading.value = false;
  if (supported) timer = setInterval(() => { void refresh(); }, 1000);
});
onUnmounted(() => { disposed = true; if (timer) clearInterval(timer); });
</script>

<template>
  <div class="clipboard-probe-switch">
    <label class="toggle-row">
      <span>{{ t("浮窗听写后自动粘贴（测试）") }}</span>
      <span class="toggle-switch-slot">
        <input type="checkbox" class="toggle-input" :checked="checked" :disabled="!supported || loading || busy" @change="change" />
        <span v-if="busy || loading" class="toggle-spinner" aria-hidden="true"></span>
      </span>
    </label>
    <p class="tiny muted">{{ t("按住语音键在浮窗接字，松开并识别完成后自动复制、返回原输入框粘贴；重启后关闭。") }}</p>
    <p v-if="error" role="alert">{{ t("接字浮窗未能准备好，请重试。") }}</p>
  </div>
</template>

<style scoped>
.clipboard-probe-switch { margin-bottom: 12px; }
p { margin: 8px 0; }
[role="alert"] { color: var(--error-text); }
</style>

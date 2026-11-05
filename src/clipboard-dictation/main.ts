import { createApp, watchEffect } from "vue";
import ClipboardDictation from "./ClipboardDictation.vue";
import { initializeLanguage } from "../lib/language";
import { initializeTheme } from "../lib/theme";
import { initializeAccentColor } from "../lib/accent";
import { installFrontendDiagnostics } from "../lib/frontend-diagnostics";
import { isTauriRuntime } from "../lib/bridge";
import { t } from "../lib/i18n";
import "../styles.css";
import "./window.css";

installFrontendDiagnostics();
void initializeLanguage();
void initializeTheme();
void initializeAccentColor();
createApp(ClipboardDictation).mount("#app");
watchEffect(() => {
  const title = t("剪贴板听写");
  document.title = title;
  if (isTauriRuntime()) {
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
      getCurrentWindow().setTitle(title),
    ).catch(() => {});
  }
});

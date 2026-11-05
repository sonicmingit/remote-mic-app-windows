import { isTauriRuntime } from "../lib/bridge";
import { t } from "../lib/i18n";

let opening: Promise<void> | null = null;

export function openClipboardDictation(): Promise<void> {
  if (opening) return opening;
  opening = open().finally(() => { opening = null; });
  return opening;
}

async function open(): Promise<void> {
  if (!isTauriRuntime()) {
    const popup = window.open("/clipboard-dictation.html", "sayall-clipboard-dictation", "width=560,height=540");
    if (!popup) throw new Error("window_unavailable");
    popup.focus();
    return;
  }
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const existing = await WebviewWindow.getByLabel("clipboard-dictation");
  if (existing) {
    await existing.unminimize();
    await existing.show();
    await existing.setFocus();
    return;
  }
  const popup = new WebviewWindow("clipboard-dictation", {
    url: "clipboard-dictation.html",
    title: t("剪贴板听写"),
    width: 560, height: 540, minWidth: 380, minHeight: 440,
    center: true, focus: true,
  });
  await new Promise<void>((resolve, reject) => {
    // This is a host-response deadline, not part of voice-key timing. If the host
    // replies late, the next click discovers and reuses the created window.
    let settled = false;
    const unlisteners: Array<() => void> = [];
    const finish = (created: boolean) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      unlisteners.forEach((unlisten) => unlisten());
      if (created) resolve(); else reject(new Error("window_unavailable"));
    };
    const timeout = setTimeout(() => finish(false), 15_000);
    for (const [event, created] of [["tauri://created", true], ["tauri://error", false]] as const) {
      void popup.once(event, () => finish(created)).then((unlisten) => {
        if (settled) unlisten(); else unlisteners.push(unlisten);
      }).catch(() => finish(false));
    }
  });
}

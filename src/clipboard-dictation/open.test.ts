import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openClipboardDictation } from "./open";

const host = vi.hoisted(() => ({
  existing: null as null | { show: ReturnType<typeof vi.fn>; unminimize: ReturnType<typeof vi.fn>; setFocus: ReturnType<typeof vi.fn> },
  create: vi.fn(),
  listeners: new Map<string, () => void>(),
}));
vi.mock("../lib/bridge", () => ({ isTauriRuntime: () => true }));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    static async getByLabel() { return host.existing; }
    constructor(label: string, options: unknown) { host.create(label, options); }
    async once(event: string, callback: () => void) {
      host.listeners.set(event, callback);
      return () => host.listeners.delete(event);
    }
  },
}));

beforeEach(() => { host.existing = null; host.create.mockClear(); host.listeners.clear(); vi.useFakeTimers(); });
afterEach(() => vi.useRealTimers());
async function untilRegistered() { for (let i = 0; i < 15; i++) await Promise.resolve(); }

describe("听写窗口生命周期", () => {
  it("reuses and focuses an existing window without clearing text", async () => {
    host.existing = { show: vi.fn(), unminimize: vi.fn(), setFocus: vi.fn() };
    await openClipboardDictation();
    expect(host.create).not.toHaveBeenCalled();
    expect(host.existing.setFocus).toHaveBeenCalledOnce();
    expect(host.existing.unminimize).toHaveBeenCalledOnce();
  });
  it("shares one in-flight creation and only resolves after the created event", async () => {
    const first = openClipboardDictation();
    expect(openClipboardDictation()).toBe(first);
    await untilRegistered();
    expect(host.create).toHaveBeenCalledWith("clipboard-dictation", expect.objectContaining({ url: "clipboard-dictation.html" }));
    host.listeners.get("tauri://created")!();
    await first;
  });
  it("releases the creation guard on failure, allowing a retry", async () => {
    const first = openClipboardDictation();
    const checked = expect(first).rejects.toThrow("window_unavailable");
    await untilRegistered();
    host.listeners.get("tauri://error")!();
    await checked;
    const retry = openClipboardDictation();
    await untilRegistered();
    host.listeners.get("tauri://created")!();
    await retry;
    expect(host.create).toHaveBeenCalledTimes(2);
  });
  it("times out a missing host response and releases the guard for another attempt", async () => {
    const first = openClipboardDictation();
    const checked = expect(first).rejects.toThrow("window_unavailable");
    await vi.advanceTimersByTimeAsync(15_000);
    await checked;
    const retry = openClipboardDictation();
    await untilRegistered();
    host.listeners.get("tauri://created")!();
    await retry;
  });
});

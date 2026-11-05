import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ClipboardProbeSwitch from "./ClipboardProbeSwitch.vue";
import { invoke } from "@tauri-apps/api/core";
import { effectiveLanguage } from "../lib/language-state";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../lib/bridge", () => ({ isTauriRuntime: () => true }));
beforeEach(() => {
  effectiveLanguage.value = "zh-Hans";
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (command) => command === "get_clipboard_probe_enabled" ? false : true);
});
describe("自动浮窗接字验证开关", () => {
  it("starts off and enables the native experiment explicitly", async () => {
    const wrapper = mount(ClipboardProbeSwitch);
    await flushPromises();
    expect((wrapper.get("input").element as HTMLInputElement).checked).toBe(false);
    await wrapper.get("input").setValue(true);
    await flushPromises();
    expect(invoke).toHaveBeenCalledWith("set_clipboard_probe_enabled", { enabled: true, english: false });
    expect((wrapper.get("input").element as HTMLInputElement).checked).toBe(true);
    expect(wrapper.text()).toContain("松开并识别完成后自动复制、返回原输入框粘贴");
    expect(wrapper.text()).toContain("重启后关闭");
    wrapper.unmount();
  });
  it("reverts the toggle when native initialization fails without exposing internal errors", async () => {
    const wrapper = mount(ClipboardProbeSwitch);
    await flushPromises();
    vi.mocked(invoke).mockRejectedValueOnce(new Error("private_system_error"));
    await wrapper.get("input").setValue(true);
    await flushPromises();
    expect((wrapper.get("input").element as HTMLInputElement).checked).toBe(false);
    expect(wrapper.get('[role="alert"]').text()).toContain("请重试");
    expect(wrapper.text()).not.toContain("private_system_error");
    wrapper.unmount();
  });
});

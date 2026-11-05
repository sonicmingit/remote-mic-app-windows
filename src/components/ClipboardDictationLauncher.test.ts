import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ClipboardDictationLauncher from "./ClipboardDictationLauncher.vue";
import { openClipboardDictation } from "../clipboard-dictation/open";
import { reportFrontendEvent } from "../lib/frontend-diagnostics";

vi.mock("../clipboard-dictation/open", () => ({ openClipboardDictation: vi.fn() }));
vi.mock("../lib/frontend-diagnostics", () => ({ reportFrontendEvent: vi.fn() }));
beforeEach(() => vi.mocked(openClipboardDictation).mockReset());

describe("剪贴板听写入口", () => {
  it("disables repeat clicks during creation and lets the user retry a failed open", async () => {
    let reject!: (reason: Error) => void;
    vi.mocked(openClipboardDictation).mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; }));
    const wrapper = mount(ClipboardDictationLauncher);
    await wrapper.get("button").trigger("click");
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
    reject(new Error("create_failed"));
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain("请重试");
    vi.mocked(openClipboardDictation).mockResolvedValueOnce();
    await wrapper.get("button").trigger("click");
    await flushPromises();
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(reportFrontendEvent).toHaveBeenCalledWith(expect.objectContaining({ result: "passed", reason: "window_created_or_focused" }));
    wrapper.unmount();
  });
});

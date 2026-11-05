import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ClipboardDictation from "./ClipboardDictation.vue";
import { effectiveLanguage } from "../lib/language-state";
import { reportFrontendEvent } from "../lib/frontend-diagnostics";

vi.mock("../lib/frontend-diagnostics", () => ({ reportFrontendEvent: vi.fn() }));

const writeText = vi.fn<(text: string) => Promise<void>>();
const wrappers: ReturnType<typeof mount>[] = [];
function render() {
  const wrapper = mount(ClipboardDictation, { attachTo: document.body });
  wrappers.push(wrapper);
  return wrapper;
}

beforeEach(() => {
  effectiveLanguage.value = "zh-Hans";
  writeText.mockReset().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
});
afterEach(() => { wrappers.splice(0).forEach((wrapper) => wrapper.unmount()); });

describe("剪贴板听写", () => {
  it("focuses its own editor and copies all committed text, including whitespace and multiple sessions", async () => {
    const wrapper = render();
    const editor = wrapper.get("textarea");
    expect(document.activeElement).toBe(editor.element);
    await editor.setValue("第一段。\n");
    await flushPromises();
    await editor.setValue("第一段。\n  第二段🙂");
    await flushPromises();
    expect(writeText.mock.calls.map(([text]) => text)).toEqual(["第一段。\n", "第一段。\n  第二段🙂"]);
    expect(wrapper.get('[role="status"]').text()).toContain("已复制全部文字");
  });

  it("never copies an IME preedit; copies the final DOM value after compositionend", async () => {
    const wrapper = render();
    const editor = wrapper.get("textarea");
    await editor.trigger("compositionstart");
    (editor.element as HTMLTextAreaElement).value = "临时识别";
    await editor.trigger("input", { isComposing: true });
    await flushPromises();
    expect(writeText).not.toHaveBeenCalled();
    editor.element.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true }));
    (editor.element as HTMLTextAreaElement).value = "最终识别。";
    editor.element.dispatchEvent(new InputEvent("input", { bubbles: true, isComposing: false }));
    await flushPromises();
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText).toHaveBeenLastCalledWith("最终识别。");
  });

  it("serializes clipboard writes so a slow earlier result cannot overwrite the latest text", async () => {
    let finishFirst!: () => void;
    writeText.mockImplementationOnce(() => new Promise((resolve) => { finishFirst = resolve; }));
    const wrapper = render();
    await wrapper.get("textarea").setValue("第一段");
    await flushPromises();
    await wrapper.get("textarea").setValue("第一段和第二段");
    await wrapper.get("textarea").setValue("第一段和第二段。最终内容");
    expect(writeText).toHaveBeenCalledTimes(1);
    finishFirst();
    await flushPromises();
    expect(writeText.mock.calls.map(([text]) => text)).toEqual(["第一段", "第一段和第二段。最终内容"]);
    expect(wrapper.get('[role="status"]').text()).toContain("已复制全部文字");
  });

  it.each(["blur", "clear", "unmount", "pagehide"])("drops queued copies and late success on %s", async (action) => {
    let finish!: () => void;
    writeText.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
    const wrapper = render();
    await wrapper.get("textarea").setValue("已经开始复制");
    await flushPromises();
    await wrapper.get("textarea").setValue("等待复制");
    if (action === "blur") window.dispatchEvent(new Event("blur"));
    if (action === "clear") await wrapper.get('[data-action="clear"]').trigger("click");
    if (action === "unmount") wrapper.unmount();
    if (action === "pagehide") window.dispatchEvent(new Event("pagehide"));
    finish();
    await flushPromises();
    expect(writeText).toHaveBeenCalledTimes(1);
    if (action !== "unmount") expect(wrapper.get('[role="status"]').text()).not.toContain("已复制全部文字");
  });

  it("does not log the dictated text or persist it across window instances", async () => {
    vi.mocked(reportFrontendEvent).mockClear();
    const wrapper = render();
    await wrapper.get("textarea").setValue("ONLY_FOR_THIS_WINDOW");
    await flushPromises();
    expect(JSON.stringify(vi.mocked(reportFrontendEvent).mock.calls)).not.toContain("ONLY_FOR_THIS_WINDOW");
    wrapper.unmount();
    const next = render();
    expect(next.get("textarea").element.value).toBe("");
  });

  it("preserves text on failure and offers a manual retry without losing editor focus", async () => {
    writeText.mockRejectedValueOnce(new Error("denied"));
    const wrapper = render();
    await wrapper.get("textarea").setValue("保留这段文字");
    await flushPromises();
    expect(wrapper.get("textarea").element.value).toBe("保留这段文字");
    expect(wrapper.get('[role="status"]').text()).toContain("复制失败");
    await wrapper.get('[data-action="copy"]').trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledTimes(2);
    expect(wrapper.get('[role="status"]').text()).toContain("已复制全部文字");
    expect(document.activeElement).toBe(wrapper.get("textarea").element);
  });

  it("clears only the editor and does not erase the clipboard", async () => {
    const wrapper = render();
    await wrapper.get("textarea").setValue("已经复制");
    await flushPromises();
    await wrapper.get('[data-action="clear"]').trigger("click");
    await flushPromises();
    expect(wrapper.get("textarea").element.value).toBe("");
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(wrapper.get('[data-action="copy"]').attributes("disabled")).toBeDefined();
  });

  it("shows a recoverable failure when the clipboard API is unavailable", async () => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
    const wrapper = render();
    await wrapper.get("textarea").setValue("还在窗口里");
    await flushPromises();
    expect(wrapper.get('[role="status"]').text()).toContain("复制失败");
    expect(wrapper.get("textarea").element.value).toBe("还在窗口里");
  });

  it("renders the workflow in English", async () => {
    effectiveLanguage.value = "en";
    const wrapper = render();
    expect(wrapper.text()).toContain("Clipboard Dictation");
    await wrapper.get("textarea").setValue("Hello");
    await flushPromises();
    expect(wrapper.get('[role="status"]').text()).toContain("All text copied");
    expect(wrapper.text()).not.toMatch(/\p{Script=Han}/u);
  });
});

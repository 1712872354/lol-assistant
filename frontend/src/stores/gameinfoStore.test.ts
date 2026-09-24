import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/backend", () => ({
  callApp: vi.fn(),
}));

import { callApp } from "@/lib/backend";
import { useGameinfoStore } from "@/stores/gameinfoStore";

describe("gameinfoStore 刷新失败错误态", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGameinfoStore.setState({ loading: false, error: null });
  });

  it("refresh 失败必须写入 error 状态（不得静默吞错）", async () => {
    vi.mocked(callApp).mockRejectedValueOnce(new Error("boom"));

    await useGameinfoStore.getState().refresh();

    const s = useGameinfoStore.getState();
    expect(s.loading).toBe(false);
    expect(s.error, "刷新失败必须给出错误态").toBeTruthy();
  });

  it("refresh 成功必须清空 error", async () => {
    useGameinfoStore.setState({ error: "旧错误" });
    vi.mocked(callApp).mockResolvedValueOnce({
      phase: "None",
      queueLabel: "",
      teams: [],
    } as never);

    await useGameinfoStore.getState().refresh();

    expect(useGameinfoStore.getState().error).toBeNull();
  });
});

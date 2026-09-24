import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { UpdateDialog } from "@/components/UpdateDialog";
import { useUpdateStore } from "@/stores/updateStore";

describe("UpdateDialog a11y", () => {
  it("打开时必须是 dialog 语义（role=dialog + aria-modal）", () => {
    useUpdateStore.setState({
      dialogOpen: true,
      info: {
        hasUpdate: true,
        currentVersion: "1.0.7",
        version: "1.0.8",
        notes: "修复若干问题",
        pubDate: "2026-09-24",
        releaseUrl: "https://github.com/1712872354/lol-assistant/releases/tag/v1.0.8",
      },
    });

    render(<UpdateDialog />);
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    // 标题可关联
    expect(dialog).toHaveAccessibleName(/发现新版本/);
  });
});

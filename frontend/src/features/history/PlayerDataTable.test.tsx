import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/AssetImg", () => ({
  AssetImg: () => <div data-testid="asset-img" />,
}));

import { PlayerDataTable, type PlayerTableRowData } from "@/features/history/PlayerDataTable";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { PlayerRow } from "@/lib/types";

function renderTable(data: PlayerTableRowData[]) {
  return render(
    <TooltipProvider>
      <PlayerDataTable data={data} />
    </TooltipProvider>,
  );
}

function mkPlayer(over: Partial<PlayerRow>): PlayerRow {
  return {
    participantId: 1,
    teamId: 100,
    placement: 0,
    puuid: "pu-1",
    summonerId: "s-1",
    name: "玩家#CN1",
    profileIconId: 1,
    championId: 22,
    champLevel: 18,
    spell1Id: 4,
    spell2Id: 14,
    runeId: 8000,
    kills: 5,
    deaths: 2,
    assists: 8,
    kda: "6.50",
    items: [1, 2, 3, 4, 5, 6, 7],
    gold: 12000,
    totalDamage: 20000,
    win: true,
    remake: false,
    augmentIds: [],
    tierShort: "",
    dmgRatio: 1.2,
    matchRating: 7.5,
    ratingRank: 1,
    killParticipation: 60,
    isSelf: false,
    ...over,
  };
}

function mkRow(over: Partial<PlayerRow>): PlayerTableRowData {
  return {
    p: mkPlayer(over),
    tone: "win",
    maxDamage: 30000,
    maxGold: 15000,
  };
}

/** 读取表格中玩家名出现顺序（每行第一个按钮=玩家名，跳过表头行） */
function nameOrder(): string[] {
  const table = screen.getByRole("table");
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((r) => within(r).getAllByRole("button")[0]?.textContent?.trim() ?? "")
    .filter(Boolean);
}

describe("PlayerDataTable 排序", () => {
  it("默认按评分降序渲染", () => {
    renderTable([
      mkRow({ name: "低分#CN1", matchRating: 3.1, ratingRank: 3 }),
      mkRow({ name: "高分#CN1", matchRating: 9.2, ratingRank: 1 }),
      mkRow({ name: "中分#CN1", matchRating: 6.0, ratingRank: 2 }),
    ]);
    expect(nameOrder()).toEqual(["高分#CN1", "中分#CN1", "低分#CN1"]);
  });

  it("点击伤害列排序按钮按伤害降序再点升序（数值列默认高值优先）", async () => {
    const user = userEvent.setup();
    renderTable([
      mkRow({ name: "低伤#CN1", totalDamage: 10000, matchRating: 3.1 }),
      mkRow({ name: "高伤#CN1", totalDamage: 30000, matchRating: 9.2 }),
      mkRow({ name: "中伤#CN1", totalDamage: 20000, matchRating: 6.0 }),
    ]);

    // 排序交互由列头内的 <button> 承载（键盘可达）
    const dmgSortBtn = screen.getByRole("button", { name: /按伤害排序/ });
    await user.click(dmgSortBtn);
    expect(nameOrder()).toEqual(["高伤#CN1", "中伤#CN1", "低伤#CN1"]);

    await user.click(dmgSortBtn);
    expect(nameOrder()).toEqual(["低伤#CN1", "中伤#CN1", "高伤#CN1"]);
  });

  it("可排序列头带 aria-sort，装备列不提供排序", () => {
    renderTable([mkRow({})]);
    // th 的可访问名 = 列名 + 列说明（sr-only），按列名开头锚定
    const dmgHeader = screen.getByRole("columnheader", { name: /^伤害/ });
    expect(dmgHeader.getAttribute("aria-sort")).toBe("none");

    // 默认按评分降序
    const ratingHeader = screen.getByRole("columnheader", { name: /^评分/ });
    expect(ratingHeader.getAttribute("aria-sort")).toBe("descending");

    const itemsHeader = screen.getByRole("columnheader", { name: "装备" });
    expect(itemsHeader.className).not.toContain("cursor-pointer");
    expect(itemsHeader.getAttribute("aria-sort")).toBeNull();
    expect(within(itemsHeader).queryByRole("button")).toBeNull();
  });

  it("本人行带 self 高亮类", () => {
    renderTable([mkRow({ isSelf: true, name: "我#CN1" })]);
    const row = screen.getByRole("row", { name: /我#CN1/ });
    expect(row.className).toContain("bg-self-bg");
  });
});

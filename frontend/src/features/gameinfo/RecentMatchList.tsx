import { AssetImg } from "@/lib/AssetImg";
import type { GameinfoRecentMatch } from "@/lib/types";
import { cn } from "@/lib/utils";
import { relTime } from "./format";

interface Props {
  recent: GameinfoRecentMatch[];
  uniformQueue: boolean;
  hiddenCareer?: boolean;
}

/** 近况列表：左侧胜负色条 + 英雄 + 时间/胜负 + KDA；队列去重 */
export function RecentMatchList({ recent, uniformQueue, hiddenCareer }: Props) {
  return (
      <div className="min-h-0 flex-1 space-y-1 overflow-y-auto p-1.5">
        {recent.map((r, i) => (
          <div
            key={i}
            className={cn(
              "flex items-center gap-2 rounded-md border-l-[3px] px-2 py-1",
              r.win
                ? "border-l-win-bar bg-win-bg/25"
                : "border-l-loss-bar bg-loss-bg/25",
            )}
          >
            <AssetImg kind="champion" id={r.championId} size={28} className="rounded-md" />
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 items-center gap-1.5">
                {!uniformQueue ? (
                  <span className="shrink-0 rounded bg-muted px-1 py-px text-[10px] text-muted-foreground">
                    {r.queueShort || "对局"}
                  </span>
                ) : (
                  <span
                    className={cn(
                      "shrink-0 text-[11px] font-semibold",
                      r.win ? "text-team-win-fg" : "text-team-loss-fg",
                    )}
                  >
                    {r.win ? "胜" : "负"}
                  </span>
                )}
                {!uniformQueue ? (
                  <span className="truncate text-[11px] font-medium">
                    {r.queueName || r.queueShort || "对局"}
                  </span>
                ) : (
                  <span className="truncate text-[11px] text-muted-foreground">
                    {relTime(r.gameCreation, r.timeShort)}
                  </span>
                )}
              </div>
              {!uniformQueue ? (
                <div className="mt-0.5 text-[10px] leading-none text-muted-foreground">
                  {relTime(r.gameCreation, r.timeShort)}
                  <span className="mx-0.5">·</span>
                  <span
                    className={cn(
                      "font-semibold",
                      r.win ? "text-team-win-fg" : "text-team-loss-fg",
                    )}
                  >
                    {r.win ? "胜" : "负"}
                  </span>
                </div>
              ) : null}
            </div>
            <div className="tnum shrink-0 text-[13px] font-semibold">
              {r.kills}/{r.deaths}/{r.assists}
            </div>
          </div>
        ))}
        {recent.length === 0 ? (
          <div className="py-8 text-center text-[11px] text-muted-foreground">
            {hiddenCareer ? "生涯隐藏 · 暂无近战数据" : "暂无近战数据"}
          </div>
        ) : null}
      </div>

  );
}

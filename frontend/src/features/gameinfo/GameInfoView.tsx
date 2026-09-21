import { Gamepad2, MonitorX } from "lucide-react";
import { useEffect } from "react";
import { useAppStore } from "@/stores/appStore";
import { useGameinfoStore } from "@/stores/gameinfoStore";
import { TeamPanel } from "./TeamPanel";

/**
 * 对局信息页：筛选控件已上移标题栏；本页只渲染双方区块与对照条。
 */
export function GameInfoView() {
  const conn = useAppStore((s) => s.conn);
  const offline = conn.state !== "connected";
  const sideFilter = useGameinfoStore((s) => s.sideFilter);
  const view = useGameinfoStore((s) => s.view);
  const refresh = useGameinfoStore((s) => s.refresh);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const empty = offline
    ? {
        icon: <MonitorX className="mb-3 h-10 w-10 text-muted-foreground/60" />,
        title: "客户端未连接",
        desc: "启动并登录英雄联盟客户端后，这里将自动展示双方玩家与近期战绩",
      }
    : view.phase === "None"
      ? {
          icon: <Gamepad2 className="mb-3 h-10 w-10 text-muted-foreground/60" />,
          title: "客户端空闲",
          desc: "进入房间或对局后，这里将自动展示双方玩家与近期战绩",
        }
      : null;

  const teams =
    sideFilter === "all"
      ? view.teams
      : view.teams.filter((t) => t.key === sideFilter);

  const allyTeam = view.teams.find((t) => t.key === "ally");
  const enemyTeam = view.teams.find((t) => t.key === "enemy");

  return (
    <div className="flex h-full min-h-0 flex-col gap-2 p-2">
      {empty ? (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-border text-center">
          {empty.icon}
          <div className="text-sm font-medium text-foreground">{empty.title}</div>
          <div className="max-w-[320px] text-xs leading-relaxed text-muted-foreground">
            {empty.desc}
          </div>
        </div>
      ) : (
        <>
          {allyTeam && enemyTeam && sideFilter === "all" ? (
            <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 rounded-xl border border-border/60 bg-card/70 px-3 py-1.5 text-[11px] text-muted-foreground">
              <span className="font-medium text-ally-fg">我方 {allyTeam.winRate.toFixed(1)}%</span>
              <div className="flex min-w-[120px] flex-1 items-center gap-1">
                <div className="h-[5px] flex-1 overflow-hidden rounded-full bg-muted/70">
                  <div
                    className="h-full rounded-full bg-ally-fg/70"
                    style={{ width: `${Math.min(100, allyTeam.winRate)}%` }}
                  />
                </div>
                <div className="h-[5px] flex-1 overflow-hidden rounded-full bg-muted/70">
                  <div
                    className="ml-auto h-full rounded-full bg-enemy-fg/70"
                    style={{ width: `${Math.min(100, enemyTeam.winRate)}%` }}
                  />
                </div>
              </div>
              <span className="font-medium text-enemy-fg">{enemyTeam.winRate.toFixed(1)}% 敌方</span>
              <span className="opacity-40">·</span>
              <span>
                评分{" "}
                <span className="tnum font-semibold text-foreground">{allyTeam.rating}</span>
                <span className="mx-1 opacity-50">vs</span>
                <span className="tnum font-semibold text-foreground">{enemyTeam.rating}</span>
              </span>
            </div>
          ) : null}

          <div className="flex min-h-0 flex-1 flex-col gap-2.5">
            {teams.map((team) => (
              <TeamPanel
                key={team.key}
                team={team}
                offline={offline}
                opponent={
                  sideFilter === "all"
                    ? team.key === "ally"
                      ? enemyTeam
                      : allyTeam
                    : undefined
                }
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

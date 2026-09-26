import { Gamepad2, MonitorX } from "lucide-react";
import { useEffect } from "react";
import { EmptyState } from "@/components/EmptyState";
import { useAppStore } from "@/stores/appStore";
import { phaseLabelCN } from "@/lib/phase";
import { useGameinfoStore } from "@/stores/gameinfoStore";
import { TeamPanel } from "./TeamPanel";

/**
 * 对局信息页：筛选控件已上移标题栏；本页只渲染双方区块与对照条 */
export function GameInfoView() {
  const conn = useAppStore((s) => s.conn);
  const offline = conn.state !== "connected";
  const sideFilter = useGameinfoStore((s) => s.sideFilter);
  const view = useGameinfoStore((s) => s.view);
  const refresh = useGameinfoStore((s) => s.refresh);
  const error = useGameinfoStore((s) => s.error);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const empty = offline
    ? {
        icon: MonitorX,
        title: "客户端未连接",
        desc: "启动并登录英雄联盟客户端后，这里将自动展示双方玩家与近期战绩",
      }
    : view.phase === "None"
      ? {
          icon: Gamepad2,
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
      {error ? (
        <div className="shrink-0 rounded-md border border-destructive/40 bg-destructive/5 px-3 py-1.5 text-xs text-destructive">
          刷新失败：{error}
        </div>
      ) : null}
      {empty ? (
        <EmptyState
          className="flex-1 border-dashed"
          icon={empty.icon}
          title={empty.title}
          desc={empty.desc}
        />
      ) : (
        <>
          <div className="flex min-h-0 flex-1 flex-col gap-2.5">
            {teams.map((team) => (
              <TeamPanel
                key={team.key}
                team={team}
                offline={offline}
                phaseLabel={phaseLabelCN(view.phase)}
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

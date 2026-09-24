import type { GameinfoTeamView } from "@/lib/types";
import { THREAT_FILL, type ThreatTone } from "@/lib/tone";
import { cn } from "@/lib/utils";
import { PlayerSlotCard } from "./PlayerSlotCard";

interface Props {
  /** 阶段文案（由 GameInfoView 以 lib/phase 单点口径传入） */
  phaseLabel: string;
  team: GameinfoTeamView;
  offline: boolean;
  /** 对方队伍统计（用于表头对照条；单侧筛选时可空） */
  opponent?: Pick<GameinfoTeamView, "winRate" | "teamScore">;
}

/** 表头统计 chip：灰底药丸 + 可选相对条 */
function StatChip({
  label,
  value,
  valueClass,
  barPct,
  barTone,
  title,
}: {
  label: string;
  value: string;
  valueClass?: string;
  barPct?: number;
  barTone?: ThreatTone;
  title?: string;
}) {
  const bar = THREAT_FILL[barTone ?? "muted"];
  return (
    <span
      title={title}
      className="flex min-w-[72px] flex-col gap-1 rounded-full bg-card/70 px-2.5 py-1"
    >
      <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
        {label}
        <span className={cn("tnum text-xs font-semibold", valueClass)}>{value}</span>
      </span>
      {barPct != null ? (
        <span className="h-[3px] w-full overflow-hidden rounded-full bg-muted/60">
          <span
            className={cn("block h-full rounded-full", bar)}
            style={{ width: `${Math.min(100, Math.max(4, barPct))}%` }}
          />
        </span>
      ) : null}
    </span>
  );
}

/** 对局页队伍区块：淡色面板 + 对照统计 + 5 卡槽 */
export function TeamPanel({ team, offline, opponent, phaseLabel }: Props) {
  const ally = team.key === "ally";
  const firstEmpty = team.slots.findIndex((s) => !s.filled);

  // 对方在场时做差值色档，单侧视图退回绝对阈值
  const wrDiff = opponent ? team.winRate - opponent.winRate : 0;
  const ratingDiff = opponent ? team.teamScore - opponent.teamScore : 0;
  const wrTone = opponent
    ? wrDiff >= 2
      ? "good"
      : wrDiff <= -2
        ? "bad"
        : "mid"
    : team.winRate >= 55
      ? "good"
      : team.winRate < 45
        ? "bad"
        : "mid";
  const ratingTone = opponent
    ? ratingDiff >= 3
      ? "good"
      : ratingDiff <= -3
        ? "bad"
        : "mid"
    : "muted";

  return (
    <section
      className={cn(
        "flex min-h-0 flex-1 flex-col rounded-2xl border p-2.5",
        ally
          ? "border-ally-border/45 bg-linear-to-b from-ally-bg/55 to-ally-bg/15"
          : "border-enemy-border/45 bg-linear-to-b from-enemy-bg/55 to-enemy-bg/15",
      )}
    >
      <header className="mb-2 flex shrink-0 flex-wrap items-center gap-x-2.5 gap-y-1 px-0.5">
        <span
          className={cn("h-1.5 w-1.5 shrink-0 rounded-full", ally ? "bg-ally-fg" : "bg-enemy-fg")}
        />
        <span className="text-[13px] font-semibold">{team.label}</span>
        <span className="text-[11px] text-muted-foreground">{team.sideText}</span>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[10px] font-medium",
            ally ? "bg-ally-soft text-ally-fg" : "bg-enemy-soft text-enemy-fg",
          )}
        >
          {team.badge}
        </span>
        <span className="text-[11px] text-muted-foreground">
          {team.playerCount} 人 · 阶段 {offline ? "—" : phaseLabel}
        </span>
        <div className="ml-auto flex items-center gap-1.5">
          <StatChip
            label="胜率"
            value={`${team.winRate.toFixed(1)}%`}
            valueClass={wrTone === "good" ? "text-good-fg" : wrTone === "bad" ? "text-destructive" : "text-foreground"}
            barPct={team.winRate}
            barTone={wrTone}
            title={opponent ? `相对对方 ${wrDiff >= 0 ? "+" : ""}${wrDiff.toFixed(1)}%` : "近况平均胜率"}
          />
          <StatChip
            label="评分"
            value={String(team.teamScore)}
            valueClass={ratingTone === "good" ? "text-good-fg" : ratingTone === "bad" ? "text-destructive" : "text-foreground"}
            barPct={Math.min(100, team.teamScore)}
            barTone={ratingTone}
            title={opponent ? `相对对方 ${ratingDiff >= 0 ? "+" : ""}${ratingDiff}` : "队伍综合评分"}
          />
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-5 gap-2.5">
        {team.slots.map((slot, i) => (
          <PlayerSlotCard
            key={`${team.key}-${i}`}
            slot={slot}
            teamKey={team.key}
            offline={offline}
            showCaption={i === firstEmpty}
          />
        ))}
      </div>
    </section>
  );
}

import { cn } from "@/lib/utils";

/** 段位徽章：单双=品牌金 / 灵活=冷蓝，同规格；LP 弱化 */
export function RankChip({
  main,
  lp,
  variant = "primary",
  title,
}: {
  main: string;
  lp: string;
  variant?: "primary" | "secondary";
  title: string;
}) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex max-w-full items-baseline gap-1 rounded px-1.5 py-[3px] leading-none",
        variant === "primary"
          ? "bg-brand-gold/15 text-brand-gold"
          : "bg-ally-bg text-ally-fg",
      )}
    >
      <span className="truncate text-[12px] font-semibold">{main}</span>
      {lp ? (
        <span className="tnum shrink-0 text-[10px] font-medium opacity-65">{lp}</span>
      ) : null}
    </span>
  );
}

import { cn } from "@/lib/utils";

/** 段位徽章：单双琥珀底 / 灵活天蓝底，同规格；LP 弱化 */
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
        "inline-flex max-w-full items-baseline gap-1 rounded-md px-1.5 py-[3px] leading-none",
        variant === "primary"
          ? "bg-amber-500/12 text-amber-800 dark:bg-amber-400/15 dark:text-amber-100"
          : "bg-sky-500/12 text-sky-800 dark:bg-sky-400/15 dark:text-sky-100",
      )}
    >
      <span className="truncate text-[12px] font-semibold">{main}</span>
      {lp ? (
        <span className="tnum shrink-0 text-[10px] font-medium opacity-65">{lp}</span>
      ) : null}
    </span>
  );
}

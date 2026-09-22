import { useEffect } from "react";
import { Download, ExternalLink, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { formatReleaseNotes, useUpdateStore } from "@/stores/updateStore";

/** 更新对话框：下载并安装 / 稍后 / 打开发布页 */
export function UpdateDialog() {
  const open = useUpdateStore((s) => s.dialogOpen);
  const info = useUpdateStore((s) => s.info);
  const installing = useUpdateStore((s) => s.installing);
  const progress = useUpdateStore((s) => s.progress);
  const error = useUpdateStore((s) => s.error);
  const quitHint = useUpdateStore((s) => s.quitHint);
  const setDialogOpen = useUpdateStore((s) => s.setDialogOpen);
  const downloadAndInstall = useUpdateStore((s) => s.downloadAndInstall);

  // done/error 后允许关闭；下载中才锁定
  const stage = progress?.stage;
  const canClose = !installing || stage === "done" || stage === "error";

  useEffect(() => {
    if (!canClose) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setDialogOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [canClose, setDialogOpen]);

  if (!open || !info?.hasUpdate) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/45 p-4">
      <div className="w-full max-w-md rounded-xl border bg-card p-5 shadow-xl">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold">发现新版本 {info.version}</h2>
            <p className="mt-0.5 text-[11px] text-muted-foreground">
              当前 {info.currentVersion || "dev"}
              {info.pubDate ? ` · 发布于 ${info.pubDate.slice(0, 10)}` : ""}
            </p>
          </div>
          <button
            type="button"
            className="text-muted-foreground hover:text-foreground"
            onClick={() => setDialogOpen(false)}
            disabled={!canClose}
            aria-label="关闭"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {info.notes ? (
          <div className="mt-3">
            <p className="mb-1 text-[11px] font-medium text-muted-foreground">本次更新</p>
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-muted/50 p-3 text-[11px] leading-relaxed text-foreground/90">
              {formatReleaseNotes(info.notes)}
            </pre>
          </div>
        ) : (
          <p className="mt-3 text-[11px] text-muted-foreground">本次更新内容暂无说明。</p>
        )}

        {progress ? (
          <div className="mt-3 space-y-1">
            <div className="h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-toggle-on transition-all"
                style={{ width: `${Math.min(100, progress.percent)}%` }}
              />
            </div>
            <p className="text-[11px] text-muted-foreground">{progress.message}</p>
          </div>
        ) : null}

        {error ? (
          <p className="mt-3 break-all text-[11px] text-destructive">{error}</p>
        ) : null}

        {quitHint ? (
          <p className="mt-3 rounded-md bg-amber-500/10 p-2 text-[11px] leading-relaxed text-amber-600 dark:text-amber-400">
            {quitHint}
          </p>
        ) : null}

        <div className="mt-4 flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            className="h-8 gap-1.5"
            disabled={installing || !info.setupUrl}
            onClick={() => void downloadAndInstall()}
          >
            <Download className="h-3.5 w-3.5" />
            {installing ? "处理中…" : "下载并安装"}
          </Button>
          {info.releaseUrl ? (
            <Button
              size="sm"
              variant="outline"
              className="h-8 gap-1.5"
              onClick={() => {
                const u = info.releaseUrl;
                if (!u || !/^https:\/\//i.test(u)) return;
                window.open(u, "_blank", "noopener,noreferrer");
              }}
            >
              <ExternalLink className="h-3.5 w-3.5" />
              发布页
            </Button>
          ) : null}
          <Button
            size="sm"
            variant="ghost"
            className="h-8"
            disabled={!canClose}
            onClick={() => setDialogOpen(false)}
          >
            稍后
          </Button>
        </div>
      </div>
    </div>
  );
}

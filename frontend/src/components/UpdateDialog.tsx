import { Download, ExternalLink } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { formatReleaseNotes, useUpdateStore } from "@/stores/updateStore";

/** 更新对话框：下载并安装 / 稍后 / 打开发布页（shadcn Dialog，官方自带焦点陷阱与 Esc） */
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
  const visible = open && !!info?.hasUpdate;

  return (
    <Dialog
      open={visible}
      onOpenChange={(v) => {
        if (!v && canClose) setDialogOpen(false);
      }}
    >
      <DialogContent
        showCloseButton={false}
        aria-modal="true"
        className="max-w-md gap-3"
        onEscapeKeyDown={(e) => {
          if (!canClose) e.preventDefault();
        }}
        onPointerDownOutside={(e) => {
          if (!canClose) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle>发现新版本 {info?.version}</DialogTitle>
          <DialogDescription>
            当前 {info?.currentVersion || "dev"}
            {info?.pubDate ? ` · 发布于 ${info.pubDate.slice(0, 10)}` : ""}
          </DialogDescription>
        </DialogHeader>

        {info?.notes ? (
          <div>
            <p className="mb-1 text-[11px] font-medium text-muted-foreground">本次更新</p>
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-muted/50 p-3 text-[11px] leading-relaxed text-foreground/90">
              {formatReleaseNotes(info.notes)}
            </pre>
          </div>
        ) : (
          <p className="text-[11px] text-muted-foreground">本次更新内容暂无说明</p>
        )}

        {progress ? (
          <div className="space-y-1">
            <Progress value={Math.min(100, progress.percent)} className="h-1.5" />
            <p className="text-[11px] text-muted-foreground">{progress.message}</p>
          </div>
        ) : null}

        {error ? (
          <p className="break-all text-[11px] text-destructive">{error}</p>
        ) : null}

        {quitHint ? (
          <p className="rounded-md bg-amber-500/10 p-2 text-[11px] leading-relaxed text-amber-600 dark:text-amber-400">
            {quitHint}
          </p>
        ) : null}

        <DialogFooter>
          <Button
            size="sm"
            className="h-8 gap-1.5"
            disabled={installing || !info?.hasUpdate}
            onClick={() => void downloadAndInstall()}
          >
            <Download className="h-3.5 w-3.5" />
            {installing ? "处理中" : "下载并安装"}
          </Button>
          {info?.releaseUrl ? (
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
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

import { useState } from "react";
import type { ReactNode } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { AppConfig, ThemeMode } from "@/lib/types";
import { useAppStore } from "@/stores/appStore";
import { useGameinfoStore } from "@/stores/gameinfoStore";

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="grid gap-1.5">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs font-medium text-muted-foreground">{label}</span>
        {children}
      </div>
      {hint ? <p className="text-[11px] leading-snug text-muted-foreground/80">{hint}</p> : null}
    </div>
  );
}

export function SettingsView() {
  const config = useAppStore((s) => s.config);
  const patchConfig = useAppStore((s) => s.patchConfig);
  const [clientPathDraft, setClientPathDraft] = useState<string | null>(null);

  const patch = (p: Partial<AppConfig>) => {
    // patchConfig 内部串行持久化；失败仅打日志，UI 保持乐观
    patchConfig(p);
    if (p.pageSize !== undefined) {
      void useGameinfoStore.getState().refresh();
    }
  };

  return (
    <div className="mx-auto flex h-full max-w-2xl flex-col gap-4 p-6">
      <div>
        <h1 className="text-base font-semibold">设置</h1>
        <p className="text-xs text-muted-foreground">外观、战绩分页与客户端连接相关选项</p>
      </div>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm">外观</CardTitle>
          <CardDescription>主题模式对界面整体生效</CardDescription>
        </CardHeader>
        <CardContent>
          <Row label="主题模式">
            <ToggleGroup
              type="single"
              size="sm"
              value={config.theme}
              onValueChange={(v) => v && patch({ theme: v as ThemeMode })}
              className="rounded-md border bg-card p-0.5"
            >
              <ToggleGroupItem value="system">跟随系统</ToggleGroupItem>
              <ToggleGroupItem value="light">亮</ToggleGroupItem>
              <ToggleGroupItem value="dark">暗</ToggleGroupItem>
            </ToggleGroup>
          </Row>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm">战绩</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <Row label="战绩数量" hint="对局页每人近况展示与统计场数（10/20/30），默认 20">
            <ToggleGroup
              type="single"
              size="sm"
              value={String(config.pageSize)}
              onValueChange={(v) => v && patch({ pageSize: Number(v) })}
              className="rounded-md border bg-card p-0.5"
            >
              {["10", "20", "30"].map((n) => (
                <ToggleGroupItem key={n} value={n}>
                  {n}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Row>
          <Separator />
          <Row label="战绩数量" hint="对局页每人近况展示与统计场数（10/20/30），默认 20">
            <ToggleGroup
              type="single"
              size="sm"
              value={String(config.careerLimit)}
              onValueChange={(v) => v && patch({ careerLimit: Number(v) })}
              className="rounded-md border bg-card p-0.5"
            >
              {["10", "20", "30"].map((n) => (
                <ToggleGroupItem key={n} value={n}>
                  {n}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          </Row>
          <Separator />
          <Row label="SGP 云端数据源" hint="段位/战绩优先经腾讯 SGP 获取，失败自动回退 LCU；仅腾讯国服生效">
            <ToggleGroup
              type="single"
              size="sm"
              value={config.sgpEnabled ? "on" : "off"}
              onValueChange={(v) => v && patch({ sgpEnabled: v === "on" })}
              className="rounded-md border bg-card p-0.5"
            >
              <ToggleGroupItem value="on">开</ToggleGroupItem>
              <ToggleGroupItem value="off">关</ToggleGroupItem>
            </ToggleGroup>
          </Row>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-sm">客户端</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <Row
            label="关闭到托盘"
            hint="开启：点关闭隐藏到托盘，右键托盘图标「显示主界面」唤回、「退出」彻底关闭；关闭：点关闭直接退出"
          >
            <ToggleGroup
              type="single"
              size="sm"
              value={config.closeToTray ? "on" : "off"}
              onValueChange={(v) => v && patch({ closeToTray: v === "on" })}
              className="rounded-md border bg-card p-0.5"
            >
              <ToggleGroupItem value="on">开</ToggleGroupItem>
              <ToggleGroupItem value="off">关</ToggleGroupItem>
            </ToggleGroup>
          </Row>
          <Separator />
          <Row label="客户端路径" hint="留空时自动检测；WeGame 场景可手动指定">
            <Input
              value={clientPathDraft ?? config.clientPath}
              onChange={(e) => setClientPathDraft(e.target.value)}
              onBlur={() => {
                if (clientPathDraft !== null && clientPathDraft !== config.clientPath) {
                  patch({ clientPath: clientPathDraft });
                }
                setClientPathDraft(null);
              }}
              placeholder="例如 D:\League of Legends"
              className="h-8 w-64 text-xs"
            />
          </Row>
        </CardContent>
      </Card>
    </div>
  );
}

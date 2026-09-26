import { useState } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldContent, FieldDescription, FieldGroup, FieldLabel, FieldSeparator } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { AppConfig, ThemeMode } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useGameinfoStore } from "@/stores/gameinfoStore";

const toggleCls = "h-8 shrink-0 rounded-md border bg-background p-0.5";

function Row({
  label,
  htmlFor,
  description,
  control,
}: {
  label: string;
  htmlFor?: string;
  description?: string;
  control: React.ReactNode;
}) {
  return (
    <Field orientation="horizontal" className="items-start gap-4">
      <FieldContent className="min-w-0 flex-1">
        <FieldLabel htmlFor={htmlFor}>{label}</FieldLabel>
        {description ? (
          <FieldDescription className="text-xs leading-relaxed">
            {description}
          </FieldDescription>
        ) : null}
      </FieldContent>
      <div className="flex shrink-0 items-start pt-0.5">{control}</div>
    </Field>
  );
}

export function SettingsView() {
  const config = useAppStore((s) => s.config);
  const patchConfig = useAppStore((s) => s.patchConfig);
  const [clientPathDraft, setClientPathDraft] = useState<string | null>(null);

  const patch = (p: Partial<AppConfig>) => {
    patchConfig(p);
    if (p.pageSize !== undefined) {
      void useGameinfoStore.getState().refresh();
    }
  };

  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-2xl flex-col gap-4 p-6">
        <div>
          <h1 className="text-base font-semibold">设置</h1>
          <p className="text-xs text-muted-foreground">
            外观、战绩分页与客户端连接相关选项
          </p>
        </div>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">外观</CardTitle>
            <CardDescription>主题模式对界面整体生效</CardDescription>
          </CardHeader>
          <CardContent>
            <FieldGroup className="gap-4">
              <Row
                label="主题模式"
                control={
                  <ToggleGroup
                    type="single"
                    size="sm"
                    value={config.theme}
                    onValueChange={(v) => v && patch({ theme: v as ThemeMode })}
                    className={toggleCls}
                  >
                    <ToggleGroupItem value="system">暗色优先</ToggleGroupItem>
                    <ToggleGroupItem value="light">亮</ToggleGroupItem>
                    <ToggleGroupItem value="dark">暗</ToggleGroupItem>
                  </ToggleGroup>
                }
              />
            </FieldGroup>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">战绩</CardTitle>
          </CardHeader>
          <CardContent>
            <FieldGroup className="gap-4">
              <Row
                label="战绩数量"
                description="对局页每人近况展示与统计场数（10/20/30），默认 20"
                control={
                  <ToggleGroup
                    type="single"
                    size="sm"
                    value={String(config.pageSize)}
                    onValueChange={(v) => v && patch({ pageSize: Number(v) })}
                    className={toggleCls}
                  >
                    {["10", "20", "30"].map((n) => (
                      <ToggleGroupItem key={n} value={n}>
                        {n}
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                }
              />
              <FieldSeparator />
              <Row
                label="生涯场数"
                description="玩家生涯统计的场数上限（10/20/30），默认 20"
                control={
                  <ToggleGroup
                    type="single"
                    size="sm"
                    value={String(config.careerLimit)}
                    onValueChange={(v) => v && patch({ careerLimit: Number(v) })}
                    className={toggleCls}
                  >
                    {["10", "20", "30"].map((n) => (
                      <ToggleGroupItem key={n} value={n}>
                        {n}
                      </ToggleGroupItem>
                    ))}
                  </ToggleGroup>
                }
              />
              <FieldSeparator />
              <Row
                label="SGP 云端数据"
                htmlFor="sgp-switch"
                description="段位/战绩优先经腾讯 SGP 获取，失败自动回退 LCU；仅腾讯国服生效"
                control={
                  <Switch
                    id="sgp-switch"
                    checked={config.sgpEnabled}
                    onCheckedChange={(v) => patch({ sgpEnabled: v })}
                  />
                }
              />
            </FieldGroup>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">客户端</CardTitle>
          </CardHeader>
          <CardContent>
            <FieldGroup className="gap-4">
              <Row
                label="关闭到托盘"
                htmlFor="tray-switch"
                description="开启：点关闭隐藏到托盘，右键托盘「显示主界面」/「退出」；关闭：点关闭直接退出"
                control={
                  <Switch
                    id="tray-switch"
                    checked={config.closeToTray}
                    onCheckedChange={(v) => patch({ closeToTray: v })}
                  />
                }
              />
              <FieldSeparator />
              <Row
                label="客户端路径"
                htmlFor="client-path"
                description="留空时自动检测；WeGame 场景可手动指定"
                control={
                  <Input
                    id="client-path"
                    value={clientPathDraft ?? config.clientPath}
                    onChange={(e) => setClientPathDraft(e.target.value)}
                    onBlur={() => {
                      if (
                        clientPathDraft !== null &&
                        clientPathDraft !== config.clientPath
                      ) {
                        patch({ clientPath: clientPathDraft });
                      }
                      setClientPathDraft(null);
                    }}
                    placeholder="例如 D:\League of Legends"
                    className={cn("h-8 w-56 text-xs", "sm:w-64")}
                  />
                }
              />
            </FieldGroup>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import { callAppStrict } from "@/lib/backend";

/** kind → (id → 名称) 进程级缓存 */
const nameCache = new Map<string, Map<number, string>>();
/** kind → 在途请求（合并并发调用） */
const inflight = new Map<string, Promise<Map<number, string>>>();

function snapshot(kind: string): Map<number, string> {
  return new Map(nameCache.get(kind) ?? []);
}

/**
 * 资源名称批量查询（无障碍替代文本用）。
 * 失败静默降级为空映射——名称是增强信息，不阻塞 UI。
 */
export function useAssetNames(kind: string, ids: number[]): Map<number, string> {
  const wanted = ids.filter((id) => id > 0);
  const key = [...wanted].sort((a, b) => a - b).join(",");
  const [names, setNames] = useState<Map<number, string>>(() => snapshot(kind));

  useEffect(() => {
    if (!key) return;
    const missing = wanted.filter((id) => !nameCache.get(kind)?.has(id));
    if (missing.length === 0) {
      setNames(snapshot(kind));
      return;
    }
    let alive = true;
    let p = inflight.get(kind);
    if (!p) {
      p = callAppStrict<Record<string, string>>("GetMatchAssetNames", kind, missing)
        .then((r) => {
          const m = nameCache.get(kind) ?? new Map<number, string>();
          // Tauri HashMap<i32,String> 序列化为对象（键为字符串）
          for (const [id, name] of Object.entries(r)) m.set(Number(id), String(name));
          nameCache.set(kind, m);
          inflight.delete(kind);
          return m;
        })
        .catch((e: unknown) => {
          inflight.delete(kind);
          throw e;
        });
      inflight.set(kind, p);
    }
    p.then((m) => {
      if (alive) setNames(new Map(m));
    }).catch(() => {
      /* 名称缺失可接受 */
    });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind, key]);

  return names;
}

import { useEffect, useState } from "react";

import { callAppStrict } from "@/lib/backend";
import type { AssetResult } from "@/lib/types";
import { cn } from "@/lib/utils";

const assetCache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();
const ASSET_CACHE_CAP = 800;

/** 资源图标 base64 → data URL（内存缓存 + in-flight 合并） */
function useAsset(kind: string, id: number): string | null {
  const key = `${kind}:${id}`;
  const [url, setUrl] = useState<string | null>(() => assetCache.get(key) ?? null);

  useEffect(() => {
    if (!id || id <= 0) {
      setUrl(null);
      return;
    }
    const hit = assetCache.get(key);
    if (hit) {
      setUrl(hit);
      return;
    }
    let alive = true;
    let p = inflight.get(key);
    if (!p) {
      p = callAppStrict<AssetResult>("GetMatchAsset", kind, id)
        .then((r) => {
          const u = `data:${r.mime};base64,${r.data}`;
          if (assetCache.size >= ASSET_CACHE_CAP) assetCache.clear();
          assetCache.set(key, u);
          inflight.delete(key);
          return u;
        })
        .catch((e) => {
          inflight.delete(key);
          throw e;
        });
      inflight.set(key, p);
    }
    p.then((u) => {
      if (alive) setUrl(u);
    }).catch(() => {
      if (alive) setUrl(null);
    });
    return () => {
      alive = false;
    };
  }, [key, kind, id]);

  return url;
}


interface AssetImgProps {
  kind: string; // champion/profile/item/spell/perk/augment
  id: number;
  size?: number;
  className?: string;
  title?: string;
}

/**
 * LCU 资源图标：经 Go 代理取 base64 → data URL。
 * id<=0 渲染空槽位；加载中渲染脉冲占位；失败保持占位（不破坏布局）。
 */
export function AssetImg({ kind, id, size = 20, className, title }: AssetImgProps) {
  const url = useAsset(kind, id);
  const style = { width: size, height: size };

  if (!id || id <= 0) {
    return (
      <span
        aria-hidden
        className={cn("inline-block shrink-0 rounded-[3px] bg-muted/60", className)}
        style={style}
      />
    );
  }
  if (!url) {
    return (
      <span
        aria-hidden
        className={cn("inline-block shrink-0 animate-pulse rounded-[3px] bg-muted", className)}
        style={style}
      />
    );
  }
  return (
    <img
      src={url}
      alt=""
      title={title}
      draggable={false}
      style={style}
      className={cn("inline-block shrink-0 rounded-[3px] object-cover", className)}
    />
  );
}

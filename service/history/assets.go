package history

// LCU 游戏资源代理：把 /lol-game-data/assets 图标以 base64 返回前端（前端拼 data URL）。
// 通路：直链（champion/profile/item）+ 资源表索引（spell/perk/augment，10 分钟 TTL 缓存）。
// 字节缓存为会话级内存缓存（上限 maxByteEntries，图标量级 ~10KB，远低于 200MB 预算）；
// 磁盘缓存属 M4 优化项（开发方案 §5.3）。

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/1712872354/lol-assistant/internal/lcu"
)

// 资源类型常量（前端 AssetImg kind 参数）
const (
	AssetChampion = "champion"
	AssetProfile  = "profile"
	AssetItem     = "item"
	AssetSpell    = "spell"
	AssetPerk     = "perk"
	AssetAugment  = "augment"
)

var errAssetUnavailable = errors.New("资源不可用")

const (
	indexTTL       = 10 * time.Minute
	maxByteEntries = 1024
)

type assetEntry struct{ mime, data string }

type indexEntry struct {
	loaded time.Time
	paths  map[int]string
}

type assetCache struct {
	mu      sync.Mutex
	bytes   map[string]assetEntry
	indexes map[string]indexEntry
}

func newAssetCache() *assetCache {
	return &assetCache{
		bytes:   map[string]assetEntry{},
		indexes: map[string]indexEntry{},
	}
}

// AssetResult 资源图标（base64；data URL 由前端拼接）
type AssetResult struct {
	Kind string `json:"kind"`
	ID   int    `json:"id"`
	Mime string `json:"mime"`
	Data string `json:"data"`
}

// GetAsset 按类型与 ID 获取图标；命中缓存直接返回
func (s *Service) GetAsset(kind string, id int) (AssetResult, error) {
	if id <= 0 {
		return AssetResult{}, fmt.Errorf("%w: 无效资源 id %d", errAssetUnavailable, id)
	}
	cli, err := s.client()
	if err != nil {
		return AssetResult{}, err
	}

	key := kind + ":" + strconv.Itoa(id)
	s.assets.mu.Lock()
	if e, ok := s.assets.bytes[key]; ok {
		s.assets.mu.Unlock()
		return AssetResult{Kind: kind, ID: id, Mime: e.mime, Data: e.data}, nil
	}
	s.assets.mu.Unlock()

	path := s.resolvePath(cli, kind, id)
	if path == "" {
		return AssetResult{}, fmt.Errorf("%w: %s %d 无图标映射", errAssetUnavailable, kind, id)
	}
	status, body, err := cli.Get(path)
	if (err != nil || status < 200 || status >= 300 || len(body) == 0) && kind == AssetItem {
		// 物品直链失败 → 回退 items.json 索引
		if p, ok := s.lookupIndex(cli, lcu.PathGDItems)[id]; ok && p != "" {
			alt := normalizeAssetPath(p)
			if st2, b2, e2 := cli.Get(alt); e2 == nil && st2 >= 200 && st2 < 300 && len(b2) > 0 {
				status, body, err, path = st2, b2, nil, alt
			}
		}
	}
	if err != nil {
		return AssetResult{}, fmt.Errorf("获取资源失败 %s/%d: %w", kind, id, err)
	}
	if status < 200 || status >= 300 || len(body) == 0 {
		return AssetResult{}, fmt.Errorf("%w: %s %d (HTTP %d)", errAssetUnavailable, kind, id, status)
	}

	mime := mimeByExt(path)
	data := base64.StdEncoding.EncodeToString(body)
	s.assets.mu.Lock()
	if len(s.assets.bytes) >= maxByteEntries {
		s.assets.bytes = map[string]assetEntry{} // 简单清空防无界增长
	}
	s.assets.bytes[key] = assetEntry{mime: mime, data: data}
	s.assets.mu.Unlock()
	return AssetResult{Kind: kind, ID: id, Mime: mime, Data: data}, nil
}

// resolvePath 资源类型 → LCU 路径；索引类资源无映射时返回空串
func (s *Service) resolvePath(cli *lcu.Client, kind string, id int) string {
	switch kind {
	case AssetChampion:
		return fmt.Sprintf(lcu.PathGDChampionIcon, id)
	case AssetProfile:
		return fmt.Sprintf(lcu.PathGDProfileIcon, id)
	case AssetItem:
		return fmt.Sprintf(lcu.PathGDItemIcon, id) // 失败时 GetAsset 内回退索引
	case AssetSpell:
		return normalizeAssetPath(s.lookupIndex(cli, lcu.PathGDSpells)[id])
	case AssetPerk:
		return normalizeAssetPath(s.lookupIndex(cli, lcu.PathGDPerks)[id])
	case AssetAugment:
		return normalizeAssetPath(s.lookupIndex(cli, lcu.PathGDAugments)[id])
	default:
		return ""
	}
}

type idxRaw struct {
	ID                    int    `json:"id"`
	IconPath              string `json:"iconPath"`
	AugmentSmallImagePath string `json:"augmentSmallImagePath"`
	AugmentLargeImagePath string `json:"augmentLargeImagePath"`
}

// lookupIndex 资源表索引：id → iconPath（TTL 缓存；拉取失败也缓存空表，避免风暴）
func (s *Service) lookupIndex(cli *lcu.Client, jsonPath string) map[int]string {
	s.assets.mu.Lock()
	if e, ok := s.assets.indexes[jsonPath]; ok && time.Since(e.loaded) < indexTTL {
		s.assets.mu.Unlock()
		return e.paths
	}
	s.assets.mu.Unlock()

	paths := map[int]string{}
	if status, body, err := cli.Get(jsonPath); err == nil && status >= 200 && status < 300 {
		var arr []idxRaw
		if json.Unmarshal(body, &arr) == nil {
			for _, e := range arr {
				p := e.IconPath
				if p == "" {
					p = e.AugmentSmallImagePath
				}
				if p == "" {
					p = e.AugmentLargeImagePath
				}
				if e.ID > 0 && p != "" {
					paths[e.ID] = p
				}
			}
		}
	}

	s.assets.mu.Lock()
	s.assets.indexes[jsonPath] = indexEntry{loaded: time.Now(), paths: paths}
	s.assets.mu.Unlock()
	return paths
}

// normalizeAssetPath 资源表 iconPath → 可请求的 LCU 路径
// 已是完整 /lol-game-data/assets 前缀直接返回；/fe/… 等 LCU Web 资源路由原样保留
func normalizeAssetPath(p string) string {
	p = strings.TrimSpace(p)
	if p == "" {
		return ""
	}
	if strings.HasPrefix(p, "http://") || strings.HasPrefix(p, "https://") {
		return p
	}
	switch {
	case strings.HasPrefix(p, "/lol-game-data/assets"):
		return p
	case strings.HasPrefix(p, "/lol-game-data/"):
		return "/lol-game-data/assets/" + strings.TrimPrefix(p, "/lol-game-data/")
	case strings.HasPrefix(p, "assets/"):
		return "/lol-game-data/" + p
	case strings.HasPrefix(p, "/fe/"):
		return p
	default:
		return "/lol-game-data/assets/" + strings.TrimPrefix(p, "/")
	}
}

// mimeByExt 扩展名 → MIME
func mimeByExt(path string) string {
	lower := strings.ToLower(path)
	switch {
	case strings.HasSuffix(lower, ".png"):
		return "image/png"
	case strings.HasSuffix(lower, ".jpg"), strings.HasSuffix(lower, ".jpeg"):
		return "image/jpeg"
	case strings.HasSuffix(lower, ".svg"):
		return "image/svg+xml"
	case strings.HasSuffix(lower, ".webp"):
		return "image/webp"
	case strings.HasSuffix(lower, ".gif"):
		return "image/gif"
	default:
		return "application/octet-stream"
	}
}

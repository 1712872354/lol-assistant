package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// ThemeMode 主题模式
type ThemeMode string

const (
	// ThemeSystem 跟随系统
	ThemeSystem ThemeMode = "system"
	// ThemeLight 亮色
	ThemeLight ThemeMode = "light"
	// ThemeDark 暗色
	ThemeDark ThemeMode = "dark"
)

// Config 应用配置（前端 AppConfig 类型与 JSON 字段一一对应）
type Config struct {
	SchemaVersion  int       `json:"schemaVersion"`
	Theme          ThemeMode `json:"theme"`
	PageSize       int       `json:"pageSize"`
	ApiConcurrency int       `json:"apiConcurrency"`
	SgpEnabled     bool      `json:"sgpEnabled"`
	CloseToTray    bool      `json:"closeToTray"`
	ClientPath     string    `json:"clientPath"`
}

const currentSchema = 1

// Default 默认配置（对应开发方案 §2.1 设置项）
func Default() Config {
	return Config{
		SchemaVersion:  currentSchema,
		Theme:          ThemeSystem,
		PageSize:       20,
		ApiConcurrency: 5,
		SgpEnabled:     true,
		CloseToTray:    true,
	}
}

// Store 配置存储：内存快照 + JSON 持久化（%APPDATA%\LOLAssistant\config.json）
type Store struct {
	mu   sync.RWMutex
	path string
	cfg  Config
}

// NewStore 创建存储，自动定位 %APPDATA%\LOLAssistant
func NewStore() *Store {
	return &Store{
		path: filepath.Join(defaultDir(), "config.json"),
		cfg:  Default(),
	}
}

func defaultDir() string {
	base, err := os.UserConfigDir()
	if err != nil || base == "" {
		return "."
	}
	return filepath.Join(base, "LOLAssistant")
}

// Path 配置文件路径（日志/诊断用）
func (s *Store) Path() string { return s.path }

// Load 读取配置。文件缺失或损坏时回退默认值；损坏文件备份为 .bad 供排查。
func (s *Store) Load() {
	s.mu.Lock()
	defer s.mu.Unlock()

	data, err := os.ReadFile(s.path)
	if err != nil {
		// 文件缺失或读取失败均不阻塞启动，按默认值运行
		s.cfg = Default()
		return
	}

	var c Config
	if err := json.Unmarshal(data, &c); err != nil {
		_ = os.WriteFile(s.path+".bad", data, 0o644)
		s.cfg = Default()
		return
	}
	s.cfg = sanitize(c)
}

// sanitize 边界校验：非法字段回退默认值（开发方案 §2.1 取值范围）
func sanitize(c Config) Config {
	d := Default()
	if c.SchemaVersion <= 0 {
		c.SchemaVersion = currentSchema
	}
	switch c.Theme {
	case ThemeSystem, ThemeLight, ThemeDark:
	default:
		c.Theme = d.Theme
	}
	if c.PageSize < 5 || c.PageSize > 50 {
		c.PageSize = d.PageSize
	}
	// 对局页聚合并发仅开放 2/5/10 三挡，旧值（4/6/8 等）归一到默认 5
	if c.ApiConcurrency < 1 || c.ApiConcurrency > 32 {
		c.ApiConcurrency = d.ApiConcurrency
	}
	// 对局页聚合并发仅开放 2/5/10 三档，旧值（4/6/8 等）归一到默认 5
	switch c.ApiConcurrency {
	case 2, 5, 10:
	default:
		c.ApiConcurrency = d.ApiConcurrency
	}
	// ClientPath 拒绝 UNC/设备路径，防 NTLM 凭据外泄
	c.ClientPath = sanitizeClientPath(c.ClientPath)
	return c
}

// sanitizeClientPath 仅接受本地绝对盘符路径；UNC/相对/空一律清空（走注册表发现）。
func sanitizeClientPath(p string) string {
	p = strings.TrimSpace(p)
	if p == "" {
		return ""
	}
	if strings.HasPrefix(p, `\\`) || strings.HasPrefix(p, `//`) {
		return ""
	}
	if len(p) < 3 || p[1] != ':' || (p[2] != '\\' && p[2] != '/') {
		return ""
	}
	if strings.ContainsAny(p, `<>|"?*`) {
		return ""
	}
	return strings.TrimRight(p, `\/`)
}

// Get 读取当前配置快照
func (s *Store) Get() Config {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.cfg
}

// Set 校验、生效并持久化配置
func (s *Store) Set(c Config) error {
	s.mu.Lock()
	s.cfg = sanitize(c)
	s.mu.Unlock()
	return s.Save()
}

// Save 将当前配置写盘（目录不存在时自动创建）
func (s *Store) Save() error {
	s.mu.RLock()
	data, err := json.MarshalIndent(s.cfg, "", "  ")
	s.mu.RUnlock()
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return fmt.Errorf("mkdir config dir: %w", err)
	}
	if err := os.WriteFile(s.path, data, 0o644); err != nil {
		return fmt.Errorf("write config: %w", err)
	}
	return nil
}

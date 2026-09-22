package lcu

// LCU HTTP 客户端：TLS 跳过自签证书校验 + Basic riot:<token> 认证 + 路径白名单 + 传输层重试。
// 对齐 Yuumi client.rs 语义：仅传输层错误（连接拒绝/超时）重试，HTTP 状态码错误不重试。

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// 传输层重试参数（var 以便测试注入短间隔）
var (
	clientMaxRetries = 3
	clientRetryDelay = 500 * time.Millisecond
)

// ErrPathDenied 路径不在白名单内
var ErrPathDenied = errors.New("api path not allowed")

// maxResponseBytes LCU 响应体上限（防异常大包打爆内存）
const maxResponseBytes = 16 << 20

// allowedAPIPrefixes 前端/服务层可调用的 LCU API 路径前缀白名单。
// 仅保留两页功能所需前缀（战绩/对局信息），其余一律拒绝，收敛攻击面。
var allowedAPIPrefixes = []string{
	"/system/",
	"/lol-summoner/",
	"/lol-match-history/",
	"/lol-ranked/",
	"/lol-gameflow/",
	"/lol-champ-select/",
	"/lol-game-data/",
	"/lol-lobby/",
	"/lol-spectator/",
	"/lol-matchmaking/",
	// SGP 补数层凭据端点（league-session-token 主源 + entitlements 兜底，仅服务层内部调用）
	"/lol-league-session/",
	"/entitlements/",
	// 海克斯强化等 LCU Web 静态图标路由（战绩明细页资源代理用，窄前缀）
	"/fe/lol-loot/",
}

// sharedHTTP 进程级共享客户端：本地回环访问，禁用代理（防系统代理劫持 127.0.0.1），
// LCU 使用自签名证书，需跳过校验。
var sharedHTTP = &http.Client{
	Timeout: 5 * time.Second,
	Transport: &http.Transport{
		TLSClientConfig: &tls.Config{InsecureSkipVerify: true}, //nolint:gosec // LCU 自签名证书，仅本机回环
		Proxy:           nil,
	},
}

// Client 绑定到一次 LCU 会话的 HTTP 客户端（连接重建时由 Monitor 重新生成）
type Client struct {
	port    uint16
	token   string
	baseURL string // 测试注入用；空则按 https://127.0.0.1:{port} 拼接
}

// NewClient 创建绑定指定端口与 token 的客户端
func NewClient(port uint16, token string) *Client {
	return &Client{port: port, token: token}
}

// Port 会话端口（诊断用）
func (c *Client) Port() uint16 { return c.port }

// AuthHeader 生成 LCU Basic 认证头（base64("riot:<token>")）
func AuthHeader(token string) string {
	return "Basic " + base64.StdEncoding.EncodeToString([]byte("riot:"+token))
}

// AllowedPath API 路径白名单校验（纯函数，可测）：先剥离 query、规范化路径，再做前缀匹配。
// 规范化可拒绝 /lol-game-data/../ 等穿越形态。
func AllowedPath(path string) bool {
	p := path
	if i := strings.IndexByte(p, '?'); i >= 0 {
		p = p[:i]
	}
	if p == "" || !strings.HasPrefix(p, "/") {
		return false
	}
	// 拒绝编码穿越与点段
	if strings.Contains(p, "%2e") || strings.Contains(p, "%2E") || strings.Contains(p, "\\") {
		return false
	}
	clean := pathClean(p)
	if clean == "" || !strings.HasPrefix(clean, "/") {
		return false
	}
	// 规范化后若与原路径差异含 ".." 语义，clean 已消除；仍要求 clean 不含残留 ".."
	if strings.Contains(clean, "..") {
		return false
	}
	for _, prefix := range allowedAPIPrefixes {
		if strings.HasPrefix(clean, prefix) {
			return true
		}
	}
	return false
}

// pathClean 规范化 URL 路径（等价 path.Clean，但保持 POSIX 语义，避免 Windows 反斜杠）
func pathClean(p string) string {
	parts := strings.Split(p, "/")
	out := make([]string, 0, len(parts))
	for _, seg := range parts {
		switch seg {
		case "", ".":
			continue
		case "..":
			if len(out) > 0 {
				out = out[:len(out)-1]
			}
		default:
			out = append(out, seg)
		}
	}
	return "/" + strings.Join(out, "/")
}

func (c *Client) url(path string) string {
	if c.baseURL != "" {
		return c.baseURL + path
	}
	return fmt.Sprintf("https://127.0.0.1:%d%s", c.port, path)
}

// Do 执行 LCU 请求：白名单校验 → Basic 认证 → 传输层错误重试。
// 返回 HTTP 状态码与响应 body；非 2xx 状态码不重试，body 交上层解析。
func (c *Client) Do(method, path string, body []byte) (int, []byte, error) {
	return c.DoContext(context.Background(), method, path, body)
}

// DoContext 同 Do，但支持取消/超时。
func (c *Client) DoContext(ctx context.Context, method, path string, body []byte) (int, []byte, error) {
	if !AllowedPath(path) {
		return 0, nil, fmt.Errorf("%w: %s", ErrPathDenied, path)
	}
	u := c.url(path)
	if _, err := url.Parse(u); err != nil {
		return 0, nil, err
	}

	var lastErr error
	for attempt := 1; attempt <= clientMaxRetries; attempt++ {
		var reader io.Reader
		if body != nil {
			reader = bytes.NewReader(body)
		}
		req, err := http.NewRequestWithContext(ctx, method, u, reader)
		if err != nil {
			return 0, nil, err
		}
		req.Header.Set("Authorization", AuthHeader(c.token))
		req.Header.Set("Accept", "application/json")
		if body != nil {
			req.Header.Set("Content-Type", "application/json")
		}

		resp, err := sharedHTTP.Do(req)
		if err != nil {
			lastErr = err
			if attempt < clientMaxRetries {
				time.Sleep(clientRetryDelay)
			}
			continue
		}
		data, rerr := io.ReadAll(io.LimitReader(resp.Body, maxResponseBytes))
		_ = resp.Body.Close()
		if rerr != nil {
			return resp.StatusCode, data, fmt.Errorf("read body: %w", rerr)
		}
		return resp.StatusCode, data, nil
	}
	return 0, nil, fmt.Errorf("lcu request failed after %d attempts: %w", clientMaxRetries, lastErr)
}

// Get GET 便捷方法
func (c *Client) Get(path string) (int, []byte, error) {
	return c.Do(http.MethodGet, path, nil)
}

// Post POST 便捷方法（body 为 JSON 原文）
func (c *Client) Post(path string, body []byte) (int, []byte, error) {
	return c.Do(http.MethodPost, path, body)
}

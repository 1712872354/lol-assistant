// Package update 基于 GitHub Releases API 的检查/下载/拉起安装包更新。
// 不引入第三方 updater；资产命名约定：*Setup*.exe（NSIS）/ *Portable*.zip + SHA256SUMS.txt
package update

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"time"
)

// 发布仓库（与 go.mod module 路径一致；fork 时改这里）
const (
	RepoOwner = "1712872354"
	RepoName  = "lol-assistant"
)

// Info 更新检查结果（JSON 给前端）
type Info struct {
	HasUpdate      bool   `json:"hasUpdate"`
	CurrentVersion string `json:"currentVersion"`
	Version        string `json:"version"`
	Notes          string `json:"notes"`
	PubDate        string `json:"pubDate"`
	SetupURL       string `json:"setupUrl"`
	PortableURL    string `json:"portableUrl"`
	SHA256         string `json:"sha256"`
	ReleaseURL     string `json:"releaseUrl"`
}

// Progress 下载/安装过程（经 Events 推前端）
type Progress struct {
	Stage   string `json:"stage"` // downloading | verifying | launching | done | error
	Percent int    `json:"percent"`
	Message string `json:"message"`
}

// ghAsset GitHub Release 资产
type ghAsset struct {
	Name               string `json:"name"`
	BrowserDownloadURL string `json:"browser_download_url"`
	Size               int64  `json:"size"`
}

type ghRelease struct {
	TagName    string    `json:"tag_name"`
	Body       string    `json:"body"`
	PublishedAt time.Time `json:"published_at"`
	HTMLURL    string    `json:"html_url"`
	Assets     []ghAsset `json:"assets"`
}

// apiBaseCandidates 官方 + 国内镜像（API 与下载共用前缀规则）
func apiURLCandidates() []string {
	path := fmt.Sprintf("/repos/%s/%s/releases/latest", RepoOwner, RepoName)
	return []string{
		"https://api.github.com" + path,
		"https://ghfast.top/https://api.github.com" + path,
		"https://mirror.ghproxy.com/https://api.github.com" + path,
	}
}

// downloadURLCandidates 把官方下载 URL 复制成镜像候选
func downloadURLCandidates(official string) []string {
	return []string{
		official,
		"https://ghfast.top/" + official,
		"https://mirror.ghproxy.com/" + official,
	}
}

// Checker 更新检查器
type Checker struct {
	Current string // 当前版本，如 v0.1.0
	Client  *http.Client
	OnProgress func(Progress)
}

func New(current string) *Checker {
	return &Checker{
		Current: strings.TrimSpace(current),
		Client:  &http.Client{Timeout: 30 * time.Second},
	}
}

func (c *Checker) emit(p Progress) {
	if c.OnProgress != nil {
		c.OnProgress(p)
	}
}

// Check 拉取最新 Release 并与当前版本比较
func (c *Checker) Check() (*Info, error) {
	rel, err := c.fetchRelease()
	if err != nil {
		return nil, err
	}
	info := buildInfo(rel, c.Current)
	return info, nil
}

// DownloadAndInstall 下载 Setup 安装包 → SHA256 校验 → 拉起安装器
func (c *Checker) DownloadAndInstall(setupURL, sha256Hex string) (string, error) {
	if setupURL == "" {
		return "", fmt.Errorf("缺少安装包地址")
	}
	c.emit(Progress{Stage: "downloading", Percent: 0, Message: "开始下载安装包"})

	tmpDir := filepath.Join(os.TempDir(), "lol-assistant-update")
	if err := os.MkdirAll(tmpDir, 0o755); err != nil {
		return "", err
	}
	dst := filepath.Join(tmpDir, "LOL助手-Setup.exe")

	if err := c.download(setupURL, dst); err != nil {
		c.emit(Progress{Stage: "error", Message: "下载失败: " + err.Error()})
		return "", err
	}

	sum, err := fileSHA256(dst)
	if err != nil {
		c.emit(Progress{Stage: "error", Message: "读取文件失败"})
		return "", err
	}
	want := strings.ToLower(strings.TrimSpace(sha256Hex))
	c.emit(Progress{Stage: "verifying", Percent: 95, Message: "校验完整性"})
	if want != "" && want != sum {
		_ = os.Remove(dst)
		msg := fmt.Sprintf("SHA256 不匹配\n期望 %s\n实际 %s", want, sum)
		c.emit(Progress{Stage: "error", Message: msg})
		return "", fmt.Errorf("sha256 mismatch")
	}

	c.emit(Progress{Stage: "launching", Percent: 100, Message: "启动安装程序"})
	if err := launch(dst); err != nil {
		c.emit(Progress{Stage: "error", Message: "启动安装失败: " + err.Error()})
		return "", err
	}
	c.emit(Progress{Stage: "done", Percent: 100, Message: "安装程序已启动"})
	return dst, nil
}

func (c *Checker) fetchRelease() (*ghRelease, error) {
	var lastErr error
	for _, u := range apiURLCandidates() {
		rel, err := c.getRelease(u)
		if err == nil {
			return rel, nil
		}
		lastErr = err
	}
	return nil, fmt.Errorf("获取最新版本失败: %w", lastErr)
}

func (c *Checker) getRelease(url string) (*ghRelease, error) {
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", fmt.Sprintf("%s/%s", RepoName, c.Current))
	resp, err := c.Client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	var rel ghRelease
	if err := json.NewDecoder(resp.Body).Decode(&rel); err != nil {
		return nil, err
	}
	if rel.TagName == "" {
		return nil, fmt.Errorf("空 release")
	}
	return &rel, nil
}

func buildInfo(rel *ghRelease, current string) *Info {
	ver := normalizeVer(rel.TagName)
	info := &Info{
		CurrentVersion: normalizeVer(current),
		Version:        ver,
		Notes:          strings.TrimSpace(rel.Body),
		PubDate:        rel.PublishedAt.Format(time.RFC3339),
		ReleaseURL:     rel.HTMLURL,
	}
	for _, a := range rel.Assets {
		n := strings.ToLower(a.Name)
		switch {
		case strings.Contains(n, "setup") || strings.Contains(n, "installer") || strings.HasSuffix(n, "-setup.exe"):
			if info.SetupURL == "" {
				info.SetupURL = a.BrowserDownloadURL
			}
		case strings.Contains(n, "portable") && strings.HasSuffix(n, ".zip"):
			if info.PortableURL == "" {
				info.PortableURL = a.BrowserDownloadURL
			}
		case n == "sha256sums.txt" || n == "checksums.txt":
			if sum, err := parseSumsRemote(a.BrowserDownloadURL, rel.Assets); err == nil {
				info.SHA256 = sum
			}
		}
	}
	// 从资产旁的 .sha256 文件兜底
	if info.SHA256 == "" {
		for _, a := range rel.Assets {
			if strings.HasSuffix(strings.ToLower(a.Name), ".sha256") && info.SetupURL != "" &&
				strings.HasPrefix(strings.ToLower(a.Name), strings.TrimSuffix(strings.ToLower(filepath.Base(info.SetupURL)), filepath.Ext(a.Name))) {
				if raw, err := fetchText(a.BrowserDownloadURL); err == nil {
					info.SHA256 = strings.ToLower(strings.Fields(strings.TrimSpace(raw))[0])
				}
			}
		}
	}
	info.HasUpdate = IsNewer(info.Version, info.CurrentVersion)
	return info
}

// parseSumsRemote 从 SHA256SUMS 文本里找 Setup 资产哈希
func parseSumsRemote(_ string, assets []ghAsset) (string, error) {
	for _, a := range assets {
		if !strings.EqualFold(a.Name, "SHA256SUMS.txt") && !strings.EqualFold(a.Name, "checksums.txt") {
			continue
		}
		text, err := fetchText(downloadURLCandidates(a.BrowserDownloadURL)[0])
		if err != nil {
			// 多镜像重试
			for _, u := range downloadURLCandidates(a.BrowserDownloadURL)[1:] {
				text, err = fetchText(u)
				if err == nil {
					break
				}
			}
		}
		if err != nil {
			return "", err
		}
		for _, line := range strings.Split(text, "\n") {
			fields := strings.Fields(strings.TrimSpace(line))
			if len(fields) < 2 {
				continue
			}
			name := strings.TrimPrefix(fields[1], "*")
			n := strings.ToLower(name)
			if strings.Contains(n, "setup") || strings.Contains(n, "installer") {
				return strings.ToLower(fields[0]), nil
			}
		}
	}
	return "", fmt.Errorf("checksums 中无安装包条目")
}

func fetchText(url string) (string, error) {
	client := &http.Client{Timeout: 20 * time.Second}
	resp, err := client.Get(url)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	b, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func (c *Checker) download(officialURL, dst string) error {
	var lastErr error
	for _, u := range downloadURLCandidates(officialURL) {
		if err := c.downloadOne(u, dst); err == nil {
			return nil
		} else {
			lastErr = err
		}
	}
	return lastErr
}

func (c *Checker) downloadOne(url, dst string) error {
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", fmt.Sprintf("%s/%s", RepoName, c.Current))
	// 下载大文件给 15min
	client := &http.Client{Timeout: 15 * time.Minute}
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	out, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer out.Close()

	total := resp.ContentLength
	var read int64
	buf := make([]byte, 32*1024)
	for {
		n, rerr := resp.Body.Read(buf)
		if n > 0 {
			if _, werr := out.Write(buf[:n]); werr != nil {
				return werr
			}
			read += int64(n)
			if total > 0 {
				pct := int(read * 100 / total)
				if pct > 90 {
					pct = 90
				}
				c.emit(Progress{Stage: "downloading", Percent: pct, Message: fmt.Sprintf("下载中 %d%%", pct)})
			}
		}
		if rerr == io.EOF {
			break
		}
		if rerr != nil {
			return rerr
		}
	}
	_ = out.Sync()
	return nil
}

func fileSHA256(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}

// launch 拉起安装包（Windows 显示 UAC/安装 UI；其它平台直接 exec）
func launch(path string) error {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "windows":
		cmd = exec.Command(path)
	default:
		cmd = exec.Command(path)
	}
	cmd.Dir = filepath.Dir(path)
	return cmd.Start()
}

// normalizeVer "v1.2.3" / "1.2.3" → "1.2.3"
func normalizeVer(v string) string {
	return strings.TrimPrefix(strings.TrimSpace(v), "v")
}

// IsNewer 判断 remote 是否比 current 新（点分数字，缺段补 0）
func IsNewer(remote, current string) bool {
	r := parseVer(remote)
	c := parseVer(current)
	for i := 0; i < 3; i++ {
		if r[i] > c[i] {
			return true
		}
		if r[i] < c[i] {
			return false
		}
	}
	return false
}

func parseVer(v string) [3]int {
	v = normalizeVer(v)
	// 去掉预发布后缀
	if i := strings.IndexAny(v, "-+"); i >= 0 {
		v = v[:i]
	}
	var out [3]int
	parts := strings.Split(v, ".")
	for i := 0; i < 3 && i < len(parts); i++ {
		n, err := strconv.Atoi(parts[i])
		if err != nil {
			// "1.0.0-beta" 已剥离；其它非数字段当 0
			n = 0
		}
		out[i] = n
	}
	return out
}

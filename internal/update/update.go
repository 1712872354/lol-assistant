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
	"net/url"
	"os"
	"path/filepath"
	"regexp"
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
	// URL GitHub API 资产内容端点（api.github.com/.../releases/assets/{id}）；
	// 国内直连 github.com 发行资产常超时，api.github.com 往往可达。
	URL  string `json:"url"`
	Size int64  `json:"size"`
}

type ghRelease struct {
	TagName     string    `json:"tag_name"`
	Body        string    `json:"body"`
	PublishedAt time.Time `json:"published_at"`
	HTMLURL     string    `json:"html_url"`
	Assets      []ghAsset `json:"assets"`
}

// apiBaseCandidates 官方 + 国内镜像（API 与下载共用前缀规则）
func apiURLCandidates() []string {
	path := fmt.Sprintf("/repos/%s/%s/releases/latest", RepoOwner, RepoName)
	return []string{
		"https://api.github.com" + path,
		"https://ghfast.top/https://api.github.com" + path,
		"https://gh-proxy.com/https://api.github.com" + path,
		"https://mirror.ghproxy.com/https://api.github.com" + path,
	}
}

// downloadURLCandidates 把官方下载 URL 复制成镜像候选
func downloadURLCandidates(official string) []string {
	return []string{
		official,
		"https://ghfast.top/" + official,
		"https://gh-proxy.com/" + official,
		"https://mirror.ghproxy.com/" + official,
	}
}

// Checker 更新检查器
type Checker struct {
	Current    string // 当前版本，如 v0.1.0
	Client     *http.Client
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
	rel, official, err := c.fetchRelease()
	if err != nil {
		return nil, err
	}
	info := buildInfo(rel, c.Current, official)
	return info, nil
}

// unwrapMirrors 剥掉国内镜像前缀，得到原始 URL 字符串。
func unwrapMirrors(raw string) string {
	s := strings.TrimSpace(raw)
	for _, p := range []string{"https://ghfast.top/", "https://gh-proxy.com/", "https://mirror.ghproxy.com/"} {
		if strings.HasPrefix(s, p) {
			s = strings.TrimPrefix(s, p)
		}
	}
	return s
}

// toOfficialGitHub 仅当原始地址是 https://github.com/... 时返回，否则空串。
func toOfficialGitHub(raw string) string {
	s := unwrapMirrors(raw)
	u, err := url.Parse(s)
	if err != nil || u.Scheme != "https" || !strings.EqualFold(u.Hostname(), "github.com") {
		return ""
	}
	return s
}

// isAllowedSetupURL 仅允许本仓库 GitHub Release 资产（可经登记镜像前缀代理）。
func isAllowedSetupURL(raw string) bool {
	s := unwrapMirrors(raw)
	u, err := url.Parse(s)
	if err != nil || u.Scheme != "https" || !strings.EqualFold(u.Hostname(), "github.com") {
		return false
	}
	if u.RawQuery != "" || u.Fragment != "" {
		return false
	}
	prefix := "/" + RepoOwner + "/" + RepoName + "/releases/download/"
	if !strings.HasPrefix(u.Path, prefix) {
		return false
	}
	if strings.Contains(u.Path, "..") {
		return false
	}
	return true
}

func isSHA256Hex(s string) bool {
	if len(s) != 64 {
		return false
	}
	for _, r := range s {
		if (r < '0' || r > '9') && (r < 'a' || r > 'f') {
			return false
		}
	}
	return true
}

// DownloadAndInstall 下载 Setup 安装包 → 强制 SHA256 校验 → 拉起安装器。
// setupURL 仅允许本仓库 Release 资产；sha256Hex 为空一律拒绝（防未校验包提权执行）。
func (c *Checker) DownloadAndInstall(setupURL, sha256Hex string) (string, error) {
	if setupURL == "" {
		return "", fmt.Errorf("缺少安装包地址")
	}
	if !isAllowedSetupURL(setupURL) {
		return "", fmt.Errorf("安装包地址不在允许列表")
	}
	want := strings.ToLower(strings.TrimSpace(sha256Hex))
	if want == "" {
		return "", fmt.Errorf("缺少 SHA256，拒绝安装未校验安装包")
	}
	if !isSHA256Hex(want) {
		return "", fmt.Errorf("SHA256 格式无效")
	}
	c.emit(Progress{Stage: "downloading", Percent: 0, Message: "开始下载安装包"})

	tmpDir := filepath.Join(os.TempDir(), "lol-assistant-update")
	if err := os.MkdirAll(tmpDir, 0o700); err != nil {
		return "", err
	}
	// 随机文件名，降低 TOCTOU/固定路径预埋风险
	out, err := os.CreateTemp(tmpDir, "setup-*.exe")
	if err != nil {
		return "", err
	}
	dst := out.Name()
	_ = out.Close()
	ok := false
	defer func() {
		if !ok {
			_ = os.Remove(dst)
		}
	}()

	if err := c.download(setupURL, dst); err != nil {
		c.emit(Progress{Stage: "error", Message: "下载失败: " + err.Error()})
		return "", err
	}

	sum, err := fileSHA256(dst)
	if err != nil {
		c.emit(Progress{Stage: "error", Message: "读取文件失败"})
		return "", err
	}
	c.emit(Progress{Stage: "verifying", Percent: 95, Message: "校验完整性"})
	if want != sum {
		msg := fmt.Sprintf("SHA256 不匹配\n期望 %s\n实际 %s", want, sum)
		c.emit(Progress{Stage: "error", Message: msg})
		return "", fmt.Errorf("sha256 mismatch")
	}
	// 校验通过后再读一次，缩小校验与启动之间的替换窗口
	sum2, err := fileSHA256(dst)
	if err != nil || sum2 != sum {
		c.emit(Progress{Stage: "error", Message: "校验后文件被篡改"})
		return "", fmt.Errorf("file changed after verify")
	}

	c.emit(Progress{Stage: "launching", Percent: 100, Message: "启动安装程序"})
	installDir := detectInstallDir()
	if err := launch(dst, installDir); err != nil {
		c.emit(Progress{Stage: "error", Message: "启动安装失败: " + err.Error()})
		return "", err
	}
	ok = true
	msg := "安装程序已启动"
	if installDir != "" {
		msg = "安装程序已启动（沿用目录 " + installDir + "）"
	}
	c.emit(Progress{Stage: "done", Percent: 100, Message: msg})
	return dst, nil
}

func (c *Checker) fetchRelease() (*ghRelease, bool, error) {
	var lastErr error
	var fromMirror *ghRelease
	for _, u := range apiURLCandidates() {
		rel, err := c.getRelease(u)
		if err != nil {
			lastErr = err
			continue
		}
		// 官方 API 结果直接采用（正文哈希可信）
		if strings.HasPrefix(u, "https://api.github.com/") {
			return rel, true, nil
		}
		// 镜像结果：尽量与官方 API 交叉比对 tag/资产名，不一致则丢弃镜像
		if off, err := c.getRelease("https://api.github.com/repos/" + RepoOwner + "/" + RepoName + "/releases/latest"); err == nil {
			if rel.TagName == off.TagName && len(rel.Assets) == len(off.Assets) {
				return off, true, nil
			}
			// 镜像与官方不一致 → 以官方为准
			return off, true, nil
		}
		if fromMirror == nil {
			fromMirror = rel
		}
		lastErr = fmt.Errorf("mirror release without official cross-check")
	}
	if fromMirror != nil {
		// 官方完全不可达时仍回落镜像（正文不可信；checksums 仍只信官方，空哈希会拒装）
		return fromMirror, false, nil
	}
	return nil, false, fmt.Errorf("获取最新版本失败: %w", lastErr)
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

func buildInfo(rel *ghRelease, current string, fromOfficial bool) *Info {
	ver := normalizeVer(rel.TagName)
	info := &Info{
		CurrentVersion: normalizeVer(current),
		Version:        ver,
		Notes:          formatNotes(rel.Body),
		PubDate:        rel.PublishedAt.Format(time.RFC3339),
		ReleaseURL:     rel.HTMLURL,
	}
	var setupName string
	for _, a := range rel.Assets {
		n := strings.ToLower(a.Name)
		switch {
		case strings.Contains(n, "setup") || strings.Contains(n, "installer") || strings.HasSuffix(n, "-setup.exe"):
			if info.SetupURL == "" {
				info.SetupURL = a.BrowserDownloadURL
				setupName = a.Name
			}
		case strings.Contains(n, "portable") && strings.HasSuffix(n, ".zip"):
			if info.PortableURL == "" {
				info.PortableURL = a.BrowserDownloadURL
			}
		}
	}
	// 哈希来源（均须官方）：① 官方 API 正文（CI 写入 SHA256SUMS，免二次请求）
	// ② api.github.com 资产端点拉 SHA256SUMS ③ 直链 github.com（国内常超时）④ sidecar
	if setupName != "" {
		if fromOfficial {
			info.SHA256 = parseSHA256FromBody(rel.Body, setupName)
		}
		if info.SHA256 == "" {
			if sum, err := parseSumsFor(setupName, rel.Assets); err == nil {
				info.SHA256 = sum
			}
		}
		if info.SHA256 == "" {
			if sum, err := parseSidecarSHA(setupName, rel.Assets); err == nil {
				info.SHA256 = sum
			}
		}
	}
	info.HasUpdate = IsNewer(info.Version, info.CurrentVersion)
	return info
}

// parseSHA256FromBody 从 Release 正文按 Setup 文件名精确提取 SHA256（CI 在「安装校验」段写入）。
// 仅官方 API 返回的正文可信；镜像正文可能被篡改，调用方须校验 fromOfficial。
func parseSHA256FromBody(body, setupName string) string {
	want := strings.ToLower(strings.TrimSpace(setupName))
	if want == "" {
		return ""
	}
	for _, line := range strings.Split(body, "\n") {
		fields := strings.Fields(strings.TrimSpace(line))
		if len(fields) < 2 {
			continue
		}
		name := strings.ToLower(strings.TrimPrefix(fields[1], "*"))
		if name != want {
			continue
		}
		sum := strings.ToLower(fields[0])
		if isSHA256Hex(sum) {
			return sum
		}
	}
	return ""
}

// isOfficialAssetAPI 仅接受本仓库的 api.github.com 资产内容端点。
func isOfficialAssetAPI(raw string) bool {
	u, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || u.Scheme != "https" || !strings.EqualFold(u.Hostname(), "api.github.com") {
		return false
	}
	prefix := "/repos/" + RepoOwner + "/" + RepoName + "/releases/assets/"
	return strings.HasPrefix(u.Path, prefix) && !strings.Contains(u.Path, "..")
}

// formatNotes Release 说明 → 更新弹窗可读纯文本（安装/校验段去掉，Markdown 语法压平）。
// 在 Go 侧清洗，前端拿到即可展示，避免旧前端或 <pre> 直接露出 Markdown。
func formatNotes(md string) string {
	s := strings.ReplaceAll(md, "\r\n", "\n")
	// 去掉「安装 / 安装校验」整段（含包表格、哈希与 CI 写入的 SHA256SUMS）
	s = dropSection(s, "安装校验")
	s = dropSection(s, "安装")
	// 代码块 → 缩进行
	s = reCodeBlock.ReplaceAllStringFunc(s, func(m string) string {
		sub := reCodeBlock.FindStringSubmatch(m)
		body := ""
		if len(sub) > 1 {
			body = sub[1]
		}
		lines := strings.Split(body, "\n")
		for i, l := range lines {
			l = strings.TrimSpace(l)
			if l != "" {
				lines[i] = "    " + l
			} else {
				lines[i] = ""
			}
		}
		return strings.Join(lines, "\n")
	})
	// 表格分隔行去掉；数据行单元格用 · 连接
	s = reTableSep.ReplaceAllString(s, "")
	s = reTableRow.ReplaceAllStringFunc(s, func(m string) string {
		row := strings.TrimSuffix(strings.TrimPrefix(strings.TrimSpace(m), "|"), "|")
		cells := strings.Split(row, "|")
		out := make([]string, 0, len(cells))
		for _, c := range cells {
			c = strings.TrimSpace(c)
			if c != "" {
				out = append(out, c)
			}
		}
		return strings.Join(out, " · ")
	})
	// 标题 / 列表 / 加粗 / 行内代码 / 分隔线
	s = reHeading.ReplaceAllString(s, "")
	s = reBullet.ReplaceAllString(s, "• ")
	s = reBold.ReplaceAllString(s, "$1")
	s = reInlineCode.ReplaceAllString(s, "$1")
	s = reHR.ReplaceAllString(s, "")
	s = reBlank.ReplaceAllString(s, "\n\n")
	s = strings.TrimSpace(s)
	if s == "" {
		return "本次更新内容暂无说明。"
	}
	return s
}

// dropSection 删除 `### <title>` 到下一标题/分隔线/文末 的整段（Go regexp 无 lookahead，按行扫描）
func dropSection(s, title string) string {
	lines := strings.Split(s, "\n")
	out := make([]string, 0, len(lines))
	skipping := false
	for _, line := range lines {
		trim := strings.TrimSpace(line)
		if !skipping {
			// 命中目标标题：允许 ### 后空格/制表
			if strings.HasPrefix(trim, "###") {
				rest := strings.TrimSpace(strings.TrimPrefix(trim, "###"))
				if rest == title {
					skipping = true
					continue
				}
			}
			out = append(out, line)
			continue
		}
		// 跳过中：遇下一标题或分隔线则结束
		if strings.HasPrefix(trim, "#") || strings.HasPrefix(trim, "---") {
			skipping = false
			out = append(out, line)
		}
	}
	return strings.Join(out, "\n")
}

var (
	reCodeBlock  = regexp.MustCompile("(?s)```[\\w-]*\\n([\\s\\S]*?)```")
	reTableSep   = regexp.MustCompile(`(?m)^\|[\s:|-]+\|$`)
	reTableRow   = regexp.MustCompile(`(?m)^\|.+\|$`)
	reHeading    = regexp.MustCompile(`(?m)^#{1,6}[ \t]+`)
	reBullet     = regexp.MustCompile(`(?m)^\s*[-*+][ \t]+`)
	reBold       = regexp.MustCompile(`\*\*([^*]+)\*\*`)
	reInlineCode = regexp.MustCompile("`([^`]+)`")
	reHR         = regexp.MustCompile(`(?m)^---+[ \t]*$`)
	reBlank      = regexp.MustCompile(`\n{3,}`)
)

// parseSumsFor 从官方 GitHub 上的 SHA256SUMS.txt 按 Setup 文件名精确取哈希。
// 取内容顺序：api.github.com 资产端点（国内可达）→ 直链 github.com。
// 镜像上的 checksums 一律不采信（防同源投毒）。
func parseSumsFor(setupName string, assets []ghAsset) (string, error) {
	wantName := strings.ToLower(strings.TrimSpace(setupName))
	if wantName == "" {
		return "", fmt.Errorf("setup name empty")
	}
	var lastErr error
	for _, a := range assets {
		if !strings.EqualFold(a.Name, "SHA256SUMS.txt") && !strings.EqualFold(a.Name, "checksums.txt") {
			continue
		}
		text, err := fetchChecksumsText(a)
		if err != nil {
			lastErr = err
			continue
		}
		for _, line := range strings.Split(text, "\n") {
			fields := strings.Fields(strings.TrimSpace(line))
			if len(fields) < 2 {
				continue
			}
			name := strings.ToLower(strings.TrimPrefix(fields[1], "*"))
			if name == wantName {
				sum := strings.ToLower(fields[0])
				if !isSHA256Hex(sum) {
					return "", fmt.Errorf("invalid sha256 in checksums")
				}
				return sum, nil
			}
		}
		lastErr = fmt.Errorf("no checksum line for %s", setupName)
	}
	if lastErr == nil {
		lastErr = fmt.Errorf("checksums 中无安装包条目")
	}
	return "", lastErr
}

// fetchChecksumsText 拉取 checksums 资产正文：优先官方 API 资产端点，失败再走官方直链。
func fetchChecksumsText(a ghAsset) (string, error) {
	if a.URL != "" && isOfficialAssetAPI(a.URL) {
		if text, err := fetchAssetText(a.URL); err == nil {
			return text, nil
		}
	}
	official := toOfficialGitHub(a.BrowserDownloadURL)
	if official == "" {
		return "", fmt.Errorf("checksums asset not on official github")
	}
	return fetchText(official)
}

// fetchAssetText 经 GitHub API 资产端点读文件内容（Accept: application/octet-stream）。
func fetchAssetText(apiURL string) (string, error) {
	if !isOfficialAssetAPI(apiURL) {
		return "", fmt.Errorf("asset api not official")
	}
	req, err := http.NewRequest(http.MethodGet, apiURL, nil)
	if err != nil {
		return "", err
	}
	req.Header.Set("Accept", "application/octet-stream")
	req.Header.Set("User-Agent", fmt.Sprintf("%s/%s", RepoName, RepoOwner))
	client := &http.Client{Timeout: 20 * time.Second}
	resp, err := client.Do(req)
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

// parseSidecarSHA 从官方 GitHub 上的 <setup>.sha256 旁路文件取哈希。
func parseSidecarSHA(setupName string, assets []ghAsset) (string, error) {
	base := strings.ToLower(setupName)
	baseNoExt := strings.TrimSuffix(base, filepath.Ext(base))
	for _, a := range assets {
		n := strings.ToLower(a.Name)
		if !strings.HasSuffix(n, ".sha256") {
			continue
		}
		if n != base+".sha256" && !strings.HasPrefix(n, baseNoExt) {
			continue
		}
		official := toOfficialGitHub(a.BrowserDownloadURL)
		if official == "" {
			continue
		}
		raw, err := fetchText(official)
		if err != nil {
			continue
		}
		fields := strings.Fields(strings.TrimSpace(raw))
		if len(fields) == 0 {
			continue
		}
		sum := strings.ToLower(fields[0])
		if isSHA256Hex(sum) {
			return sum, nil
		}
	}
	return "", fmt.Errorf("no official sidecar sha256")
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

// maxSetupBytes 安装包体积上限（防异常大文件打满磁盘）
const maxSetupBytes = 200 << 20

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
	if resp.ContentLength > maxSetupBytes {
		return fmt.Errorf("install package too large: %d", resp.ContentLength)
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
			read += int64(n)
			if read > maxSetupBytes {
				return fmt.Errorf("install package too large")
			}
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

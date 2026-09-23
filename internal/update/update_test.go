package update

import (
	"strings"
	"testing"
)

func TestIsAllowedSetupURL(t *testing.T) {
	okURLs := []string{
		"https://github.com/1712872354/lol-assistant/releases/download/v1.0.2/LOLAssistant-Setup-1.0.2.exe",
		"https://ghfast.top/https://github.com/1712872354/lol-assistant/releases/download/v1.0.2/LOLAssistant-Setup-1.0.2.exe",
	}
	for _, u := range okURLs {
		if !isAllowedSetupURL(u) {
			t.Fatalf("isAllowedSetupURL(%q) = false", u)
		}
	}
	badURLs := []string{
		"",
		"http://github.com/1712872354/lol-assistant/releases/download/v1/a.exe",
		"https://evil.com/1712872354/lol-assistant/releases/download/v1/a.exe",
		"https://github.com/other/repo/releases/download/v1/a.exe",
		"https://github.com/1712872354/lol-assistant/releases/download/v1/../../../evil.exe",
		"https://github.com/1712872354/lol-assistant/releases/download/v1/a.exe?x=1",
		"https://attacker.com/a.exe",
		"file:///C:/Windows/System32/cmd.exe",
	}
	for _, u := range badURLs {
		if isAllowedSetupURL(u) {
			t.Fatalf("isAllowedSetupURL(%q) = true, want false", u)
		}
	}
}

func TestIsSHA256Hex(t *testing.T) {
	if !isSHA256Hex(strings.Repeat("ab", 32)) {
		t.Fatal("valid hex rejected")
	}
	if isSHA256Hex("") || isSHA256Hex("abc") || isSHA256Hex(strings.Repeat("g", 64)) {
		t.Fatal("invalid hex accepted")
	}
}

// 回归：SHA256 直链（github.com）超时时，哈希必须能从官方 Release 正文解析，
// 否则 Check 返回空 sha256 → 安装被「缺少 SHA256」拒绝。
func TestParseSHA256FromBody(t *testing.T) {
	sum := strings.Repeat("ab", 32)
	body := "### 修复\r\n\r\n- x\r\n\r\n### 安装校验\r\n\r\n```text\r\n" +
		sum + "  LOLAssistant-Setup-1.0.5.exe\r\n```\r\n"
	if got := parseSHA256FromBody(body, "LOLAssistant-Setup-1.0.5.exe"); got != sum {
		t.Fatalf("body hash = %q want %q", got, sum)
	}
	if got := parseSHA256FromBody(body, "other.exe"); got != "" {
		t.Fatalf("name mismatch should miss, got %q", got)
	}
	if got := parseSHA256FromBody(body, "LOLAssistant-Setup-1.0.5.exe\x00"); got != "" {
		t.Fatalf("invalid hash should miss, got %q", got)
	}
}

func TestBuildInfo_BodySHA256WithoutAssetFetch(t *testing.T) {
	sum := strings.Repeat("cd", 32)
	rel := &ghRelease{
		TagName: "v2.0.0",
		Body: "### 修复\r\n\r\n- x\r\n\r\n### 安装校验\r\n\r\n```text\r\n" +
			sum + "  LOLAssistant-Setup-2.0.0.exe\r\n```\r\n",
		Assets: []ghAsset{{
			Name:               "LOLAssistant-Setup-2.0.0.exe",
			BrowserDownloadURL: "https://github.com/1712872354/lol-assistant/releases/download/v2.0.0/LOLAssistant-Setup-2.0.0.exe",
		}},
	}
	info := buildInfo(rel, "v1.0.0", true)
	if info.SHA256 != sum {
		t.Fatalf("sha256=%q want %q（正文哈希应免二次网络）", info.SHA256, sum)
	}
	if !info.HasUpdate || info.Version != "2.0.0" {
		t.Fatalf("update meta: %+v", info)
	}
}

func TestIsOfficialAssetAPI(t *testing.T) {
	ok := "https://api.github.com/repos/1712872354/lol-assistant/releases/assets/123"
	if !isOfficialAssetAPI(ok) {
		t.Fatalf("official asset api rejected: %s", ok)
	}
	bad := []string{
		"",
		"http://api.github.com/repos/1712872354/lol-assistant/releases/assets/1",
		"https://evil.com/repos/1712872354/lol-assistant/releases/assets/1",
		"https://api.github.com/repos/other/repo/releases/assets/1",
		"https://github.com/1712872354/lol-assistant/releases/download/v1/x.txt",
	}
	for _, u := range bad {
		if isOfficialAssetAPI(u) {
			t.Fatalf("bad asset api accepted: %s", u)
		}
	}
}

func TestDownloadAndInstallRejectsEmptyHash(t *testing.T) {
	c := New("v0.0.1")
	u := "https://github.com/1712872354/lol-assistant/releases/download/v1/LOLAssistant-Setup-1.exe"
	if _, err := c.DownloadAndInstall(u, ""); err == nil {
		t.Fatal("empty sha256 should be rejected")
	}
	if _, err := c.DownloadAndInstall("https://evil.com/a.exe", strings.Repeat("a", 64)); err == nil {
		t.Fatal("disallowed url should be rejected")
	}
}

func TestSanitizeInstallDir(t *testing.T) {
	if got := sanitizeInstallDir(`C:\Program Files\LOL助手`); got != `C:\Program Files\LOL助手` {
		t.Fatalf("spaced dir = %q", got)
	}
	if got := sanitizeInstallDir(`C:\evil /S`); got != "" {
		t.Fatalf("token inject = %q", got)
	}
	if got := sanitizeInstallDir(`\\attacker\share\lol`); got != "" {
		t.Fatalf("unc = %q", got)
	}
	if got := sanitizeInstallDir(`..\relative`); got != "" {
		t.Fatalf("relative = %q", got)
	}
	if got := installerArgs(`C:\evil /S`); got != "" {
		t.Fatalf("installerArgs inject = %q", got)
	}
}

func TestIsNewer(t *testing.T) {
	cases := []struct {
		remote, current string
		want            bool
	}{
		{"1.0.1", "1.0.0", true},
		{"v1.1.0", "v1.0.9", true},
		{"1.0.0", "1.0.0", false},
		{"0.9.9", "1.0.0", false},
		{"2.0.0", "1.9.9", true},
		{"1.0", "1.0.0", false},
		{"1.0.1-beta", "1.0.0", true},
	}
	for _, c := range cases {
		if got := IsNewer(c.remote, c.current); got != c.want {
			t.Fatalf("IsNewer(%s,%s)=%v want %v", c.remote, c.current, got, c.want)
		}
	}
}

func TestParseVer(t *testing.T) {
	v := parseVer("v1.2.3-rc1")
	if v != [3]int{1, 2, 3} {
		t.Fatalf("parseVer = %v", v)
	}
}

func TestFormatNotes(t *testing.T) {
	md := "### 安装\r\n\r\n| 包 | 说明 |\r\n|----|------|\r\n| `Setup-1.0.0.exe` | NSIS 安装包 |\r\n\r\nWindows x64 · 需 WebView2\r\n\r\n### 修复\r\n\r\n- **战绩数量真正生效**：不再写死 20\r\n\r\n### 安装校验\r\n\r\n```powershell\r\nGet-FileHash x\r\n```\r\n\r\n---\r\n"
	got := formatNotes(md)
	if strings.Contains(got, "### ") || strings.Contains(got, "|") || strings.Contains(got, "```") {
		t.Fatalf("markdown leaked: %q", got)
	}
	if strings.Contains(got, "安装包") || strings.Contains(got, "Get-FileHash") {
		t.Fatalf("install section not dropped: %q", got)
	}
	if !strings.Contains(got, "战绩数量真正生效") || !strings.Contains(got, "不再写死 20") {
		t.Fatalf("fix section lost: %q", got)
	}
	if !strings.Contains(got, "• 战绩数量真正生效") {
		t.Fatalf("bullet not normalized: %q", got)
	}
}

func TestFormatNotesEmpty(t *testing.T) {
	if got := formatNotes("   "); got != "本次更新内容暂无说明。" {
		t.Fatalf("empty notes = %q", got)
	}
}

func TestInstallerArgs(t *testing.T) {
	if got := installerArgs(""); got != "" {
		t.Fatalf("empty dir = %q", got)
	}
	if got := installerArgs(`"C:\Program Files\LOL助手\"`); got != `/D=C:\Program Files\LOL助手` {
		t.Fatalf("dir = %q", got)
	}
	if got := installerArgs(`D:\Apps\LOL助手`); got != `/D=D:\Apps\LOL助手` {
		t.Fatalf("dir = %q", got)
	}
}

func TestParentOf(t *testing.T) {
	cases := map[string]string{
		`C:\Program Files\LOL助手\LOL助手.exe`:       `C:\Program Files\LOL助手`,
		`C:\Program Files\LOL助手\LOL助手.exe,0`:     `C:\Program Files\LOL助手`,
		`"C:\Program Files\LOL助手\uninstall.exe"`: `C:\Program Files\LOL助手`,
	}
	for in, want := range cases {
		if got := parentOf(in); got != want {
			t.Fatalf("parentOf(%q)=%q want %q", in, got, want)
		}
	}
}

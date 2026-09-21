package lcu

import "testing"

// 典型 LeagueClientUx 命令行（WeGame/官方启动器形态，含引号与多参数）
const sampleCmdline = `"D:\League of Legends\LeagueClientUx.exe" --riotclient-auth-token=xxx ` +
	`--riotclient-app-port=51423 --app-port=54321 --remoting-auth-token=AbCdEf123 ` +
	`--rso_platform_id=TENCENT --install-directory="D:\League of Legends" --locale=zh_CN`

func TestParseCmdline_Full(t *testing.T) {
	port, token, platformID, ok := ParseCmdline(sampleCmdline)
	if !ok {
		t.Fatal("expected ok")
	}
	if port != 54321 || token != "AbCdEf123" || platformID != "TENCENT" {
		t.Fatalf("port=%d token=%q platform=%q", port, token, platformID)
	}
}

func TestParseCmdline_QuotedToken(t *testing.T) {
	cmd := `LeagueClientUx.exe --app-port=12345 --remoting-auth-token="tok with edge" --rso_platform_id="HN1"`
	port, token, platformID, ok := ParseCmdline(cmd)
	if !ok || port != 12345 {
		t.Fatalf("ok=%v port=%d", ok, port)
	}
	// 正则在引号处截断
	if token != "tok" {
		t.Fatalf("token=%q", token)
	}
	if platformID != "HN1" {
		t.Fatalf("platform=%q", platformID)
	}
}

func TestParseCmdline_MissingToken(t *testing.T) {
	if _, _, _, ok := ParseCmdline("--app-port=54321"); ok {
		t.Fatal("token missing should fail")
	}
}

func TestParseCmdline_MissingPort(t *testing.T) {
	if _, _, _, ok := ParseCmdline("--remoting-auth-token=abc"); ok {
		t.Fatal("port missing should fail")
	}
}

func TestParseCmdline_NoPlatformID(t *testing.T) {
	port, token, platformID, ok := ParseCmdline("--app-port=1000 --remoting-auth-token=tok")
	if !ok || port != 1000 || token != "tok" || platformID != "" {
		t.Fatalf("port=%d token=%q platform=%q ok=%v", port, token, platformID, ok)
	}
}

func TestSanitizeCmdline_MasksToken(t *testing.T) {
	s := SanitizeCmdline(sampleCmdline)
	if contains(s, "AbCdEf123") {
		t.Fatalf("token leaked: %s", s)
	}
	if !contains(s, "--remoting-auth-token=***") {
		t.Fatalf("mask missing: %s", s)
	}
	if !contains(s, "--app-port=54321") {
		t.Fatalf("non-secret args must survive: %s", s)
	}
}

func contains(s, sub string) bool {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}

package lcu

import "testing"

func TestParseLockfile_Valid(t *testing.T) {
	info, ok := ParseLockfile("LeagueClient:12345:54321:AbCdEfGh123:https")
	if !ok {
		t.Fatal("expected ok")
	}
	if info.Name != "LeagueClient" || info.PID != 12345 || info.Port != 54321 ||
		info.Password != "AbCdEfGh123" || info.Protocol != "https" {
		t.Fatalf("unexpected parse result: %+v", info)
	}
}

func TestParseLockfile_NoProtocol(t *testing.T) {
	info, ok := ParseLockfile("LeagueClient:100:2000:tok")
	if !ok {
		t.Fatal("expected ok for 4-part lockfile")
	}
	if info.Protocol != "" || info.Port != 2000 {
		t.Fatalf("unexpected: %+v", info)
	}
}

func TestParseLockfile_WhitespaceTrimmed(t *testing.T) {
	info, ok := ParseLockfile("  LeagueClient:7:9999:pw:https \r\n")
	if !ok || info.PID != 7 || info.Port != 9999 {
		t.Fatalf("trim failed: ok=%v %+v", ok, info)
	}
}

func TestParseLockfile_Invalid(t *testing.T) {
	cases := []string{
		"",
		"hello",
		"LeagueClient:abc:54321:tok", // pid 非数字
		"LeagueClient:123:port:tok",  // port 非数字
		"LeagueClient:0:54321:tok",   // pid=0
		"LeagueClient:123:0:tok",     // port=0
		"LeagueClient:123:54321:",    // 空 token
		"a:b:c:d:e",                  // b/c 非数字
		"LeagueClient:123",           // 字段不足
	}
	for _, c := range cases {
		if _, ok := ParseLockfile(c); ok {
			t.Errorf("expected reject for %q", c)
		}
	}
}

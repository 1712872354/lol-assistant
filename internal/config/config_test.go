package config

import "testing"

func TestSanitizeClientPath(t *testing.T) {
	if got := sanitizeClientPath(`C:\Riot Games\League of Legends`); got != `C:\Riot Games\League of Legends` {
		t.Fatalf("local path = %q", got)
	}
	for _, p := range []string{
		`\\attacker\share\lol`,
		`//attacker/share/lol`,
		`..\relative`,
		``,
		`C:foo`,
		`C:\evil|dir`,
	} {
		if got := sanitizeClientPath(p); got != "" {
			t.Fatalf("sanitizeClientPath(%q) = %q, want empty", p, got)
		}
	}
}

func TestSanitizeAppliesClientPath(t *testing.T) {
	c := Default()
	c.ClientPath = `\\evil\share`
	c = sanitize(c)
	if c.ClientPath != "" {
		t.Fatalf("UNC ClientPath not cleared: %q", c.ClientPath)
	}
}

package update

import "testing"

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

package liveclient

import (
	"encoding/json"
	"testing"
)

func TestTeamID_Unmarshal(t *testing.T) {
	cases := []struct {
		in   string
		want TeamID
	}{
		{`100`, TeamBlue},
		{`200`, TeamRed},
		{`100.0`, TeamBlue}, // 浮点字面量
		{`200.0`, TeamRed},
		{`"100"`, TeamBlue},
		{`"200"`, TeamRed},
		{`"100.0"`, TeamBlue}, // 数字字符串小数形式 → 100
		{`"BLUE"`, TeamBlue},
		{`"RED"`, TeamRed},
		{`"blue"`, TeamBlue},  // 大小写容错
		{`"ORDER"`, TeamBlue}, // 别名编码
		{`"CHAOS"`, TeamRed},
		{`" order "`, TeamBlue}, // 空白容错
		{`0`, TeamNone},
		{`"XX"`, TeamNone},
		{`null`, TeamNone},
	}
	for _, c := range cases {
		var got TeamID
		if err := json.Unmarshal([]byte(c.in), &got); err != nil {
			t.Fatalf("unmarshal %s: %v", c.in, err)
		}
		if got != c.want {
			t.Errorf("TeamID(%s)=%d want %d", c.in, got, c.want)
		}
	}
}

func TestPlayer_Name(t *testing.T) {
	// 新版本：riotIdGameName/TagLine 优先
	p := Player{RiotIdGameName: "歪比", RiotIdTagLine: "CN1", SummonerName: "旧名#OLD"}
	if g, tag := p.Name(); g != "歪比" || tag != "CN1" {
		t.Fatalf("riotId path: %q %q", g, tag)
	}
	// 老版本：summonerName = "name#tag"
	p = Player{SummonerName: "小名#60021"}
	if g, tag := p.Name(); g != "小名" || tag != "60021" {
		t.Fatalf("summonerName split: %q %q", g, tag)
	}
	// 无 tag
	p = Player{SummonerName: "纯名"}
	if g, tag := p.Name(); g != "纯名" || tag != "" {
		t.Fatalf("plain name: %q %q", g, tag)
	}
}

package lcu

import (
	"testing"
	"time"
)

func TestParseWampEvent_Valid(t *testing.T) {
	frame := `[8,"OnJsonApiEvent_lol-gameflow-v1-gameflow-phase",` +
		`{"uri":"/lol-gameflow/v1/gameflow-phase","eventType":"Update","data":"InProgress"}]`
	evt, ok := parseWampEvent([]byte(frame))
	if !ok {
		t.Fatal("expected ok")
	}
	if evt.URI != "/lol-gameflow/v1/gameflow-phase" || evt.EventType != "Update" {
		t.Fatalf("evt=%+v", evt)
	}
	if string(evt.Data) != `"InProgress"` {
		t.Fatalf("data=%s", evt.Data)
	}
}

func TestParseWampEvent_ObjectData(t *testing.T) {
	frame := `[8,"OnJsonApiEvent_x",{"uri":"/lol-champ-select/v1/session","eventType":"Update",` +
		`{"data":{"actions":[],"localPlayerCellId":3}}]`
	// 注意上面构造有误的防御：data 应为对象字段，修正为合法帧
	frame = `[8,"OnJsonApiEvent_x",{"uri":"/lol-champ-select/v1/session","eventType":"Update","data":{"localPlayerCellId":3}}]`
	evt, ok := parseWampEvent([]byte(frame))
	if !ok {
		t.Fatal("expected ok")
	}
	if string(evt.Data) != `{"localPlayerCellId":3}` {
		t.Fatalf("data=%s", evt.Data)
	}
}

func TestParseWampEvent_Invalid(t *testing.T) {
	cases := []string{
		``,
		`not-json`,
		`[3,{},1]`,                       // 非 8 操作码（订阅回执）
		`[8,"OnJsonApiEvent"]`,           // 元素不足
		`[8,"x","not-object"]`,           // 第三元素非对象
		`[8,"x",{"eventType":"Update"}]`, // 缺 uri
	}
	for _, c := range cases {
		if _, ok := parseWampEvent([]byte(c)); ok {
			t.Errorf("should reject %q", c)
		}
	}
}

func TestUriWatched(t *testing.T) {
	watched := []string{
		"/lol-gameflow/v1/gameflow-phase",
		"/lol-gameflow/v1/session",
		"/lol-champ-select/v1/session",
		"/lol-summoner/v1/current-summoner",
		"/lol-matchmaking/v1/ready-check",
		"/lol-lobby/v2/lobby",
	}
	for _, u := range watched {
		if !uriWatched(u) {
			t.Errorf("should watch %s", u)
		}
	}
	unwatched := []string{
		"/lol-chat/v1/me",
		"/lol-honor-v2/v1/ballot",
	}
	for _, u := range unwatched {
		if uriWatched(u) {
			t.Errorf("should ignore %s", u)
		}
	}
}

func TestThrottle_Window(t *testing.T) {
	now := time.Unix(1000, 0)
	th := newThrottle(500 * time.Millisecond)
	th.now = func() time.Time { return now }

	if !th.allow() {
		t.Fatal("first frame must pass")
	}
	now = now.Add(200 * time.Millisecond)
	if th.allow() {
		t.Fatal("frame within 500ms window must be dropped")
	}
	now = now.Add(400 * time.Millisecond) // 累计 600ms
	if !th.allow() {
		t.Fatal("frame after window must pass")
	}
}

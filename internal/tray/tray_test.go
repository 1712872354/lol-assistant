package tray

import (
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// 阻塞的 OnShow 不得阻塞 OnQuit 分发（ClickedCh 无缓冲且库侧非阻塞投递，
// 消费循环一旦卡住，后续点击全部被丢弃 → 托盘“假死”）。
func TestConsumeClicks_ShowBlocked_QuitStillDispatched(t *testing.T) {
	show := make(chan struct{})
	quit := make(chan struct{})
	block := make(chan struct{})
	showStarted := make(chan struct{})
	quitDone := make(chan struct{})

	go consumeClicks(show, quit, func() {
		close(showStarted)
		<-block
	}, func() {
		close(quitDone)
	})

	// 第一次 show：异步执行并阻塞（等待消费循环调度就绪）
	sendWhenReady(t, show)
	waitFor(t, time.Second, func() bool {
		select {
		case <-showStarted:
			return true
		default:
			return false
		}
	})

	// show 仍阻塞时，quit 必须能送达
	sendWhenReady(t, quit)
	select {
	case <-quitDone:
	case <-time.After(time.Second):
		t.Fatal("quit not dispatched while show blocked")
	}

	close(block)
}

// handler panic 不得杀死消费循环（库侧 default 丢弃：无接收者 = 托盘永久无响应）。
func TestConsumeClicks_HandlerPanic_LoopSurvives(t *testing.T) {
	show := make(chan struct{})
	quit := make(chan struct{})
	quitDone := make(chan struct{})
	var showCalls atomic.Int32

	go consumeClicks(show, quit, func() {
		showCalls.Add(1)
		panic("boom")
	}, func() {
		close(quitDone)
	})

	sendWhenReady(t, show)
	waitFor(t, time.Second, func() bool { return showCalls.Load() >= 1 })

	sendWhenReady(t, show)
	waitFor(t, time.Second, func() bool { return showCalls.Load() >= 2 })

	sendWhenReady(t, quit)
	select {
	case <-quitDone:
	case <-time.After(time.Second):
		t.Fatal("quit not dispatched after handler panic")
	}
}

// 收到 quit 后消费循环必须退出，避免 goroutine 泄漏。
func TestConsumeClicks_QuitStopsLoop(t *testing.T) {
	show := make(chan struct{})
	quit := make(chan struct{})
	var wg sync.WaitGroup
	wg.Add(1)

	go consumeClicks(show, quit, func() {}, func() { wg.Done() })

	sendWhenReady(t, quit)
	waitGroupTimeout(t, &wg, time.Second)

	// 循环已退出：show 无人消费（库侧会走 default 丢弃），此处只验证不再 panic
	select {
	case show <- struct{}{}:
	default:
	}
}

// sendWhenReady 模拟 systray 库的非阻塞投递，但会等待消费 goroutine 调度就绪，
// 避免测试启动竞态（go 调度器尚未切到接收方时 default 误报假死）。
func sendWhenReady(t *testing.T, ch chan struct{}) {
	t.Helper()
	deadline := time.Now().Add(time.Second)
	for time.Now().Before(deadline) {
		select {
		case ch <- struct{}{}:
			return
		default:
			time.Sleep(5 * time.Millisecond)
		}
	}
	t.Fatal("click channel has no receiver (consumption loop stuck?)")
}

func waitFor(t *testing.T, d time.Duration, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(d)
	for time.Now().Before(deadline) {
		if cond() {
			return
		}
		time.Sleep(5 * time.Millisecond)
	}
	t.Fatal("condition not met before deadline")
}

func waitGroupTimeout(t *testing.T, wg *sync.WaitGroup, d time.Duration) {
	t.Helper()
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(d):
		t.Fatal("wait group timeout")
	}
}

//! 队列映射表（对齐 Go internal/parser/queue.go，国服实测表）。

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueInfo {
    pub id: i32,
    pub name: String,
    pub short: String,
    pub map: String,
    pub arena: bool,
}

fn q(id: i32, name: &str, short: &str, map: &str, arena: bool) -> QueueInfo {
    QueueInfo {
        id,
        name: name.into(),
        short: short.into(),
        map: map.into(),
        arena,
    }
}

/// 队列映射表（国服优先；未收录 ID 走 fallback）
fn queue_table() -> &'static std::collections::HashMap<i32, QueueInfo> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<i32, QueueInfo>> = OnceLock::new();
    TABLE.get_or_init(build_queue_table)
}

fn build_queue_table() -> std::collections::HashMap<i32, QueueInfo> {
    use std::collections::HashMap;
    let mut m = HashMap::new();
    let entries: &[(i32, &str, &str, &str, bool)] = &[
        (400, "征召模式", "征召", "召唤师峡谷", false),
        (420, "排位单双排", "单双", "召唤师峡谷", false),
        (430, "匹配模式", "匹配", "召唤师峡谷", false),
        (440, "排位灵活组排", "灵活", "召唤师峡谷", false),
        (480, "快速模式", "快速", "召唤师峡谷", false),
        (490, "快速模式", "快速", "召唤师峡谷", false),
        (450, "极地大乱斗", "大乱斗", "嚎哭深渊", false),
        (2400, "海克斯大乱斗", "海斗", "嚎哭深渊", false),
        (2450, "经典海斗", "海斗", "嚎哭深渊", false),
        (800, "人机对战", "人机", "召唤师峡谷", false),
        (810, "人机对战", "人机", "召唤师峡谷", false),
        (820, "人机对战", "人机", "嚎哭深渊", false),
        (830, "人机对战", "人机", "召唤师峡谷", false),
        (840, "人机对战", "人机", "召唤师峡谷", false),
        (850, "人机对战", "人机", "召唤师峡谷", false),
        (900, "无限火力", "火力", "召唤师峡谷", false),
        (1010, "随机无限火力", "火力", "嚎哭深渊", false),
        (1020, "克隆模式", "克隆", "召唤师峡谷", false),
        (1300, "极限闪击", "闪击", "极限闪击", false),
        (1700, "斗魂竞技场", "竞技场", "斗魂竞技场", true),
        (1710, "斗魂竞技场", "竞技场", "斗魂竞技场", true),
        (1810, "捉鬼模式", "捉鬼", "捉鬼模式", false),
        (1820, "捉鬼模式", "捉鬼", "捉鬼模式", false),
        (1830, "捉鬼模式", "捉鬼", "捉鬼模式", false),
        (1840, "捉鬼模式", "捉鬼", "捉鬼模式", false),
        (4300, "经典模式", "经典", "召唤师峡谷", false),
        (4310, "经典模式", "经典", "召唤师峡谷", false),
        (0, "自定义模式", "自定义", "自定义", false),
    ];
    for &(id, name, short, map, arena) in entries {
        m.insert(id, q(id, name, short, map, arena));
    }
    m
}

pub fn lookup_queue(queue_id: i32) -> Option<QueueInfo> {
    queue_table().get(&queue_id).cloned()
}

pub fn queue_info_for(queue_id: i32) -> QueueInfo {
    if let Some(qi) = lookup_queue(queue_id) {
        return qi;
    }
    QueueInfo {
        id: queue_id,
        name: format!("其他模式 {queue_id}"),
        short: queue_id.to_string(),
        map: "未知".into(),
        arena: false,
    }
}

#[cfg(test)]
mod t34_tests {
    use super::*;

    /// T3.4 回归：队列表必须构建一次复用（静态表），不得每次查询重建。
    #[test]
    fn queue_table_is_built_once() {
        let a: &'static std::collections::HashMap<i32, QueueInfo> = queue_table();
        let b: &'static std::collections::HashMap<i32, QueueInfo> = queue_table();
        assert!(std::ptr::eq(a, b), "queue_table 必须返回同一静态实例");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_info_for_cn_queues() {
        let cases = [
            (2400, "海克斯大乱斗", "海斗", false),
            (2450, "经典海斗", "海斗", false),
            (420, "排位单双排", "单双", false),
            (440, "排位灵活组排", "灵活", false),
            (450, "极地大乱斗", "大乱斗", false),
            (1700, "斗魂竞技场", "竞技场", true),
            (1710, "斗魂竞技场", "竞技场", true),
        ];
        for (id, name, short, arena) in cases {
            let qi = queue_info_for(id);
            assert_eq!(qi.name, name, "id={id}");
            assert_eq!(qi.short, short, "id={id}");
            assert_eq!(qi.arena, arena, "id={id}");
        }
    }

    #[test]
    fn queue_info_for_unknown_fallback() {
        let qi = queue_info_for(999999);
        assert_eq!(qi.id, 999999);
        assert_eq!(qi.short, "999999");
    }
}

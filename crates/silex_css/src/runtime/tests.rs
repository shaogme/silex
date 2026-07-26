//! 运行时状态机的回归测试。
//!
//! 这里测的**不是**「调了哪个 web API」，而是 `registry.rs` / `dynamic.rs` 里那层
//! 谁都看不见的状态机：哪张表进文档、什么时候进、退休之后还能不能捡回来、借不到
//! 注册表时欠下的活有没有补上。这些判断没有一条需要浏览器在场，而它们正是审查
//! 报告 P2 段落的重灾区——改坏了没有任何东西会响。
//!
//! 后端换成了 `runtime/fake.rs`：每一次调用都进事件日志，微任务由测试自己决定
//! 什么时候跑（`platform::run_microtasks`），于是「同一微任务内增删抵消」这类
//! 时序能被精确地摆出来。

use crate::layers;
use crate::runtime::{
    backend::SheetBackend,
    dynamic::{self, CACHE_LIMIT, DynamicStyleManager},
    fake::{self, Knobs, SheetEvent},
    platform,
    registry::{self, StaticStyleRegistry, inject_style, with_document_registry},
};

/// 每个测试都从一张白纸开始：thread_local 会在 `--test-threads=1` 下串场。
fn setup() {
    dynamic::reset_for_test();
    registry::reset_for_test();
    platform::reset();
    fake::reset();
}

fn created_sheets() -> usize {
    fake::events()
        .iter()
        .filter(|e| matches!(e, SheetEvent::Created(_)))
        .count()
}

// ---- 退休 LRU ----

/// 退休的表必须立刻摘出 adoptedStyleSheets，但表对象与内容都留着。
///
/// 报告 P2-9：退休状态曾以 `Rc` 留在队列里 → `Drop` 不触发 → `remove_sheet`
/// 不执行 → 最多 128 张表常驻文档，每一张都参与样式匹配。
#[test]
fn a_retired_sheet_leaves_the_document_but_keeps_its_content() {
    setup();
    let mut mgr = DynamicStyleManager::new();
    mgr.update("a", ".a{color:red}");
    platform::run_microtasks();
    assert_eq!(fake::adopted_now(), vec![0]);

    drop(mgr);
    platform::run_microtasks();

    assert!(fake::adopted_now().is_empty(), "退休之后不该再参与样式匹配");
    let log = fake::sheet_log(0);
    assert_eq!(
        log.content, ".a{color:red}",
        "内容要留着，复用时不必重新解析"
    );
    assert!(!log.dropped, "退休不是销毁");
}

/// 从退休队列里复用时必须挂回去，否则样式看着「在」却不生效。
#[test]
fn reusing_a_retired_sheet_reattaches_it() {
    setup();
    let mut first = DynamicStyleManager::new();
    first.update("a", ".a{color:red}");
    drop(first);
    platform::run_microtasks();
    assert!(fake::adopted_now().is_empty());

    let mut second = DynamicStyleManager::new();
    second.update("a", ".a{color:blue}");
    platform::run_microtasks();

    assert_eq!(created_sheets(), 1, "同一个 id 应当复用退休的那张表");
    assert_eq!(fake::adopted_now(), vec![0], "复用之后要挂回文档");
    assert_eq!(fake::sheet_log(0).content, ".a{color:blue}");
}

/// 超过上限时最老的那张才真正 `Drop`。
#[test]
fn the_retired_queue_evicts_in_fifo_order() {
    setup();
    let mut managers = Vec::new();
    for i in 0..=CACHE_LIMIT {
        let mut m = DynamicStyleManager::new();
        m.update(&format!("s{i}"), ".x{color:red}");
        managers.push(m);
    }
    platform::run_microtasks();
    assert_eq!(created_sheets(), CACHE_LIMIT + 1);

    // Vec 从前往后析构，于是 0 号最先进退休队列，最后一个进来时把它挤出去
    drop(managers);
    platform::run_microtasks();

    assert!(fake::sheet_log(0).dropped, "最老的那张要被真正 Drop");
    for id in 1..=CACHE_LIMIT {
        assert!(
            !fake::sheet_log(id).dropped,
            "{id} 号还在队列里，不该被 Drop"
        );
    }
    assert!(
        fake::adopted_now().is_empty(),
        "退休的表一张都不该留在文档上"
    );
}

/// 同一个 id 反复更新走的是就地替换，不该建新表、也不该动文档。
#[test]
fn updating_the_same_id_replaces_content_in_place() {
    setup();
    let mut mgr = DynamicStyleManager::new();
    mgr.update("a", ".a{color:red}");
    platform::run_microtasks();
    let syncs = fake::adopted_history().len();

    mgr.update("a", ".a{color:blue}");
    platform::run_microtasks();

    assert_eq!(created_sheets(), 1);
    assert_eq!(fake::sheet_log(0).content, ".a{color:blue}");
    assert_eq!(fake::adopted_history().len(), syncs, "内容变了但表没变");
}

// ---- adoptedStyleSheets 的同步判据 ----

/// 同一微任务内增删抵消 → 不该触发 `set_adopted_style_sheets`。
#[test]
fn a_no_op_batch_does_not_touch_the_document() {
    setup();
    let mut kept = DynamicStyleManager::new();
    kept.update("kept", ".k{color:red}");
    platform::run_microtasks();
    let before = fake::adopted_history().len();

    // 建了就退休，两笔操作落在同一批里
    let mut transient = DynamicStyleManager::new();
    transient.update("t", ".t{color:red}");
    drop(transient);
    platform::run_microtasks();

    assert_eq!(
        fake::adopted_history().len(),
        before,
        "这一批的净变化是零，不该再写一次文档"
    );
    assert_eq!(fake::adopted_now(), vec![0]);
}

/// 但增删数量相等而**内容**变了，必须同步。
///
/// 报告 P2-1：按 Rust 侧内存地址比对时这一条会被误判成「没变」——新元素可能
/// 正好落在被移除元素的槽位上。
#[test]
fn an_equal_sized_but_different_batch_is_still_synced() {
    setup();
    let mut a = DynamicStyleManager::new();
    a.update("a", ".a{color:red}");
    platform::run_microtasks();
    assert_eq!(fake::adopted_now(), vec![0]);

    let mut b = DynamicStyleManager::new();
    b.update("b", ".b{color:red}");
    drop(a);
    platform::run_microtasks();

    assert_eq!(fake::adopted_now(), vec![1], "换了一张表，文档必须跟着换");
    drop(b);
}

/// 静态表永远排在最前：层序声明在它里面，后面的表都靠它定优先级。
#[test]
fn the_static_sheet_is_always_the_first_adopted_sheet() {
    setup();
    let mut mgr = DynamicStyleManager::new();
    mgr.update("d", ".d{color:red}"); // 动态表先建，拿到 0 号
    inject_style("s", ".s{color:red}"); // 静态表后建，拿到 1 号
    platform::run_microtasks();

    assert_eq!(fake::adopted_now(), vec![1, 0]);
    drop(mgr);
}

// ---- 静态表的增量注入 ----

/// 同一个 id 注入两次只进一次表。
#[test]
fn the_same_static_id_is_only_injected_once() {
    setup();
    inject_style("a", ".a{color:red}");
    inject_style("a", ".a{color:red}");
    platform::run_microtasks();

    assert_eq!(fake::sheet_log(0).rules, vec![".a{color:red}".to_string()]);
}

/// `insertRule` 失败 → 退回整表重建，且**此前所有** chunk 都要在。
#[test]
fn a_failed_incremental_append_falls_back_to_a_full_rebuild() {
    setup();
    fake::set_knobs(Knobs {
        append_fails: true,
        ..Default::default()
    });

    inject_style("a", ".a{color:red}");
    inject_style("b", ".b{color:blue}");
    platform::run_microtasks();

    // 隔一个微任务再来一条：重建必须把先前的 chunk 一并带上
    inject_style("c", ".c{color:lime}");
    platform::run_microtasks();

    let content = fake::sheet_log(0).content;
    assert!(
        content.starts_with(layers::ORDER_STATEMENT),
        "层序声明必须还是第一条：{content}"
    );
    for chunk in [".a{color:red}", ".b{color:blue}", ".c{color:lime}"] {
        assert!(content.contains(chunk), "重建丢了 {chunk}：{content}");
    }
}

/// `<style>` 兜底没有句柄，一辈子不进 adoptedStyleSheets，但内容照样要写进去。
#[test]
fn a_style_tag_fallback_never_joins_adopted_stylesheets() {
    setup();
    fake::set_knobs(Knobs {
        tag_fallback: true,
        ..Default::default()
    });

    let mut mgr = DynamicStyleManager::new();
    mgr.update("a", ".a{color:red}");
    platform::run_microtasks();

    assert!(fake::adopted_now().is_empty());
    assert!(
        fake::adopted_history().is_empty(),
        "没有句柄就没有可同步的东西"
    );
    assert_eq!(fake::sheet_log(0).content, ".a{color:red}");

    drop(mgr);
    platform::run_microtasks();
    assert!(
        !fake::sheet_log(0).detached,
        "退休只是移出文档；兜底表要等真正 Drop 才摘"
    );
}

// ---- 借用冲突时欠下的活 ----

/// 借用冲突时的注入不能丢，下一次拿到锁要补做。
#[test]
fn a_deferred_injection_is_replayed() {
    setup();
    // 注入过程中又触发注入：`StaticStyleRegistry` 正被借着
    StaticStyleRegistry::with(|_| {
        inject_style("inner", ".inner{color:red}");
    });
    platform::run_microtasks();

    assert!(
        fake::sheet_log(0).content.contains(".inner{color:red}"),
        "借不到注册表的那次注入被丢了"
    );
}

/// 借用冲突时的挂载同样不能丢。
///
/// 此前 `attach()` 借不到文档注册表就直接 return，这张表要等到「退休之后又被
/// 复用」才会重试——在那之前它的样式一直不生效。
#[test]
fn a_deferred_attach_is_replayed() {
    setup();
    let mut held = None;
    with_document_registry(|_| {
        let mut mgr = DynamicStyleManager::new();
        mgr.update("a", ".a{color:red}");
        held = Some(mgr);
    });
    platform::run_microtasks();

    assert_eq!(fake::adopted_now(), vec![0]);
    drop(held);
}

/// 借用冲突时的摘除也不能丢。
#[test]
fn a_deferred_removal_is_replayed() {
    setup();
    let mut mgr = DynamicStyleManager::new();
    mgr.update("a", ".a{color:red}");
    platform::run_microtasks();
    assert_eq!(fake::adopted_now(), vec![0]);

    with_document_registry(|_| {
        drop(mgr);
    });
    platform::run_microtasks();

    assert!(
        fake::adopted_now().is_empty(),
        "借不到注册表的那次摘除被丢了，表会永久留在文档上"
    );
}

// ---- 建表失败 ----

/// 两条构造路都走不通时报一声就算了，不 panic、也不假装成功。
#[test]
fn a_sheet_that_cannot_be_created_is_reported_not_panicked() {
    setup();
    fake::set_knobs(Knobs {
        create_fails: true,
        ..Default::default()
    });

    inject_style("s", ".s{color:red}");
    let mut mgr = DynamicStyleManager::new();
    mgr.update("d", ".d{color:red}");
    platform::run_microtasks();

    let reports = platform::take_reports();
    assert!(
        reports.iter().any(|r| r.contains("无法创建静态样式表")),
        "{reports:?}"
    );
    assert!(
        reports.iter().any(|r| r.contains("无法建立动态样式表")),
        "{reports:?}"
    );
    assert!(fake::adopted_history().is_empty());
    assert_eq!(created_sheets(), 0);
}

/// 后端一建就成时，`create()` 的返回值确实是能用的表——防止假实现自己空转。
#[test]
fn the_fake_backend_hands_out_distinct_handles() {
    setup();
    let first = crate::runtime::backend::ActiveSheet::create().expect("建表");
    let second = crate::runtime::backend::ActiveSheet::create().expect("建表");
    assert_ne!(first.adopted(), second.adopted());
}

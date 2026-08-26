+++
title = "开发者文档"
description = "按 crate 组织的 Silex 源码、API 与设计说明。"
template = "section.html"
sort_by = "weight"
+++

开发者文档从仓库源码和测试出发，解释 crate 的职责、公开接口、生命周期边界与验证方法。

当前已完成 [`silex`](crates/silex/)、[`silex_core`](crates/silex_core/)、[`silex_reactivity`](crates/silex_reactivity/)、[`silex_rx`](crates/silex_rx/)、[`silex_dom`](crates/silex_dom/)、[`silex_view`](crates/silex_view/)、[`silex_bootstrap`](crates/silex_bootstrap/)、[`silex_html`](crates/silex_html/)、[`silex_css`](crates/silex_css/)、[`silex_macros`](crates/silex_macros/)、[`silex_net`](crates/silex_net/)、[`silex_router`](crates/silex_router/)、[`silex_i18n`](crates/silex_i18n/) 与 [`silex_i18n_macros`](crates/silex_i18n_macros/) 的总文档和专题文档；其他 crate 会按相同目录规则逐步加入。

跨 crate 的浏览器验收流程见[如何进行 Wasm 测试](wasm-testing/)。

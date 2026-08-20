+++
title = "样式表运行时与清理"
description = "silex_css 的静态 registry、动态样式表、层次顺序、CSSOM 兜底和 owner 清理。"
weight = 20
+++

# 样式表运行时与清理

`silex_css::runtime` 将 CSS 文本的生成与文档级副作用分开。静态规则进入
共享的 `StaticStyleRegistry`，动态规则由 `DynamicStyleManager` 按逻辑 ID
和内容管理。两条路径都通过 owner 生命周期或显式 `dispose` 释放资源，
避免组件重复挂载时不断累积 CSSOM 规则。

## 静态注入路径

`inject_style(id, content)` 是静态 CSS 的低层入口：

```text
inject_style(id, css)
        │
        ▼
StaticStyleRegistry
  ├── id 去重
  ├── pending chunk
  └── 下一微任务 flush
        │
        ├── append top-level rules
        └── 失败时整表 rebuild
```

第一次注入创建共享样式表，并把
`@layer base, components, utilities, overrides;` 放在第一条规则。后续
chunk 默认按顶层规则切分后追加；当 backend 没有增量接口或 `insertRule`
拒绝某条规则时，registry 会以历史 chunk 和当前 pending chunk 整体重建。
注入请求遇到 `RefCell` 重入不会静默丢失，而是进入 deferred queue，在
后续微任务补做。

`id` 是去重契约，不是 CSS 选择器。宏编译器为同一静态来源生成稳定 ID，
调用方手写 `inject_style` 时必须让同一 ID 对应相同内容；如果需要独立
生命周期或不同内容，应使用不同 ID。

## 动态样式表路径

`DynamicStyleManager::update(id, content)` 管理一张可变样式表：

1. 当前 manager 持有同一逻辑 ID 且没有其他 lease 时，原地替换内容。
2. 相同逻辑 ID、相同内容的 manager 可以共享状态。
3. 相同逻辑 ID 但内容不同必须创建独立表，避免一个 owner 覆盖另一个 owner。
4. manager 更新到新状态前会释放旧 lease；最后一个 lease 释放时，样式表
   退出文档并进入有限的退休缓存。
5. 退休表保留已解析对象供复用，但不再参与样式匹配；超过缓存上限后才
   drop，并从 registry 注销。

动态选择器使用 `DynamicCss` 或 `dynamic_rule_class`：

```text
Rx selector/value
      │ owner effect
      ▼
取值 → dynamic class hash → render selector
      │
      ├── DynamicStyleManager::update
      └── element.classList.add(new), remove(old)
```

动态声明值通常不需要新规则：`Style` 和 `DynamicCss::with_var` 将值写成
元素上的 CSS 自定义属性，再由 effect 更新 inline style。动态选择器、
全局样式或没有可挂载元素的规则只能使用动态样式表。

## CSS layer 顺序

`layers` 模块固定提供四层：

| 层 | 适用内容 |
| --- | --- |
| `base` | reset、元素默认值和全局基础样式。 |
| `components` | 组件或 `styled!` 生成的组件规则。 |
| `utilities` | `tw!` 和 utility 类。 |
| `overrides` | `sty()` 针对单个元素生成的局部覆盖。 |

静态 registry 的共享表首先注入 layer order statement，动态规则使用
`layers::wrap_dynamic` 带上所属层。`Style::render` 把局部 builder 规则放到
`overrides`，保证它作为单元素覆盖入口；不要通过注入顺序猜测 layer 优先级。

## 浏览器后端与 `<style>` 兜底

wasm 后端优先创建 `web_sys::CssStyleSheet` 并维护
`document.adoptedStyleSheets`。`sheet.rs` 的 backend 不能使用构造式样式表
时，会创建 `<style>` 插入 `<head>`；这种表没有 adopted sheet handle，
但仍由同一 registry/manager 的 attach/detach 语义管理。

文档 registry 只在必要时重新设置 `adoptedStyleSheets`，比较的是后端的
JS 对象标识，而不是 Rust `Vec` 元素地址。registry 借用冲突时，增删操作
进入统一队列并安排微任务，避免样式“偶尔不出现”或 retired sheet 永久
留在文档中。

native backend 是测试观察窗：它记录创建、replace、append、detach、drop
和 adopted snapshot，但不模拟浏览器的 CSS 解析和布局。不要以 native fake
backend 的通过结果替代 wasm browser 测试。

## 清理与错误边界

应用样式时，`Style`/`DynamicCss` 会在同一个 `MountOwnerToken` 下注册：

- reactive effect；
- 所有者拥有的 class 删除；
- 动态 CSS variable 删除；
- 动态样式表 manager 的 `dispose`。

effect 读取失败或样式表创建/替换失败时返回 `SilexError`，并通过 mount
error handler 报告。`DynamicStyleManager::dispose` 是幂等的；`Drop` 也会
尝试退休当前 lease，但需要同步知道清理是否成功时应显式调用 `dispose`
并保留上层 mount 的错误报告。

维护 runtime 时应保证以下不变量：

- 样式表只在输入 source 完成 owner/runtime 验证后注入；
- 一个 lease 释放不能摘掉仍被其他 manager 使用的共享表；
- 退休表从文档移除，但可复用的后端对象不应提前 drop；
- deferred queue 在 registry 重入结束后最终执行，不覆盖操作的先后顺序；
- owner dispose 后旧 effect 不再写入元素，也不再把旧动态 class 加回去。

这些边界对应 `src/runtime/registry.rs`、`src/runtime/dynamic.rs`、
`src/runtime/template.rs` 的状态机测试，以及 `tests/owner.rs` 和
`tests/fallback.rs` 的浏览器测试。

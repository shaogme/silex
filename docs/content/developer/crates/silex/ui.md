+++
title = "Tailwind UI 与主题"
description = "silex::ui 的 shadcn 风格组件、响应式 props 和主题入口。"
weight = 30
+++

# Tailwind UI 与主题

`silex::ui` 是基于 Tailwind class 和 shadcn 风格 token 的应用层组件集合。它
不是独立的组件 runtime：组件通过 `#[component]`/`styled!` 生成普通
`View<'scope>`，通过 `Signal`、`Callback` 和 owner-bound `AnyView` 接收输入。

## 启用与样式基础

`ui` 模块由 `tw` feature gate 控制；`tw` 会同时启用 `css` 和
`silex_css/tw`。默认 feature 已包含 `css` 与 `tw`，关闭默认 feature 时必须
显式启用：

```toml
[dependencies]
silex = { version = "...", default-features = false, features = ["css", "tw"] }
```

组件中的 `tw!`/`tw_variants!` 在编译期产生 class，动态 `Signal<String>` 只
负责在 owner scope 内合并额外 class 或状态。页面通常还应调用：

```rust
silex::ui::inject_shadcn_base_styles();
```

该函数注入 shadcn 的 base/preflight 和 CSS variables。它是浏览器 CSS runtime
入口，不会替代 `Style` 的 owner cleanup，也不会为不在页面中使用的组件自动
加载资源。

## 组件目录

组件按源码模块分组，所有带 `Ctx` 的函数都需要一个实现
`SilexContextProvider<'scope>` 的 `#[ctx]` 参数；带 `children` 的组件通过
`AnyView<'scope>` 接收子内容：

| 模块 | 公开组件 | 主要输入/语义 |
| --- | --- | --- |
| `button` | `Button` | `variant`、`size`、`class`；生成 button，不附带 click callback。 |
| `alert` | `Alert`、`AlertTitle`、`AlertDescription` | `variant`（default/destructive）和分层内容。 |
| `badge` | `Badge` | `variant`（default/secondary/destructive/outline/ghost/link）。 |
| `card` | `Card`、`CardHeader`、`CardTitle`、`CardDescription`、`CardAction`、`CardContent`、`CardFooter` | 结构化卡片布局。 |
| `avatar` | `Avatar`、`AvatarImage`、`AvatarFallback`、`AvatarBadge`、`AvatarGroup`、`AvatarGroupCount` | 尺寸、图片、fallback 和分组标记。 |
| `input` / `textarea` | `Input`、`Textarea` | `value`/`placeholder`/`type` 与 input callback；`Textarea` 是 styled view。 |
| `checkbox` / `switch` | `Checkbox`、`Switch` | `checked`、`on_change`、可选尺寸和 class。 |
| `toggle` | `Toggle` | `pressed`、`variant`、`size`、`on_change`，并输出 `data-state`。 |
| `progress` / `slider` | `Progress`、`Slider` | 数值型 signal；`Progress` 将 value 限制到 100，`Slider` 处理 min/max/step/orientation。 |
| `radio_group` | `RadioGroup`、`RadioGroupItem` | orientation、selected value、disabled、select/change callback。 |
| `tabs` | `Tabs`、`TabsList`、`TabsTrigger`、`TabsContent` | `active_tab` signal 与 trigger/content value。 |
| `accordion` | `Accordion`、`AccordionItem`、`AccordionTrigger`、`AccordionContent` | item value、trigger open signal 和 click callback。 |
| `dialog` | `Dialog`、`DialogHeader`、`DialogTitle`、`DialogDescription`、`DialogFooter` | `open` signal、close callback，内容通过 Portal。 |
| `popover` | `Popover`、`PopoverTrigger`、`PopoverContent`、`PopoverAnchor`、`PopoverClose`、`PopoverPortal`、`PopoverHeader`、`PopoverTitle`、`PopoverDescription` | `PopoverContext` 管理 open 和 anchor/content bounds。 |
| `tooltip` | `TooltipProvider`、`Tooltip`、`TooltipTrigger`、`TooltipContent` | `TooltipContext` 管理 hover/focus、anchor 和延迟关闭 timer。 |
| `separator` / `skeleton` | `Separator`、`Skeleton` | orientation 分隔线和 pulse 占位内容。 |

大多数组件的可选 props 都带 `#[chain(default)]` 和 `#[prop(into)]`：普通值会
在当前 owner 中 promotion 为 `Signal`/`Callback`，已有 owner-bound 句柄也可
直接传入。builder 的 Result 必须传播：

```rust
let button = Button(ctx, "Save")
    .variant("outline")?
    .size("sm")?
    .build()?;
```

上面的片段是 API 契约示意；它依赖外层 `ctx` 和 scope，完整可编译 facade 示例
见总文档。`Button` 本身只生成样式和内容；点击行为应通过底层 DOM event API
或业务组件的 callback prop 添加。

## 状态型组件的边界

UI 组件把状态输入和状态变更 callback 分开，避免内部偷偷创建全局状态：

- `Checkbox`/`Switch`/`Toggle` 根据当前 signal 计算 class 或 `data-state`，点击
  时调用 `on_change`；调用方负责把新值写回 signal；
- `Input` 通过 property 更新当前 value，并在 input 事件中把字符串交给
  `on_input`；HTML attribute 与 property 的区别见
  [`silex_dom 属性`](@/developer/crates/silex_dom/attributes.md)；
- `TabsTrigger` 调用 `on_select(value)`，`TabsContent` 读取 `active_tab`，两者
  不共享隐式 singleton；
- `RadioGroupItem` 通过 `selected_value` 计算 checked 状态，并把选择交给
  `on_select`/`on_change`；
- `Slider` 同时提供 hidden range input 和 pointer track，输入值经
  `min`/`max`/`step` 计算后交给 `on_change`。

这些组件的 callback、signal 和 event listener 都绑定当前 owner。owner close
后 callback 不应再调用应用代码；测试应覆盖关闭期间的事件和 stale handle。

## `PopoverContext` 与 `TooltipContext`

### Popover

`PopoverContext::new(owner)` 创建 `open`、`anchor_rect` 和 `content_height`
三个 `RwSignal`。`PopoverTrigger`/`PopoverAnchor` 更新 anchor bounds，
`PopoverContent` 根据 `side`、`align` 和 `side_offset` 计算 fixed Portal 的
位置；`PopoverClose` 负责关闭并调用可选 callback。没有浏览器 layout 的 native
测试不能验证 bounds 计算，只能检查类型和 builder。

### Tooltip

`TooltipContext::new(owner)` 创建 `open`、`anchor` 和一个可取消的 timer。
`on_pointer_enter` 会取消关闭 timer 并打开，`on_pointer_leave` 会安排默认
150ms grace period；`open`/`close`/`toggle` 是显式控制入口。timer 由 mount
owner 保存，`cancel_close_timer` 可安全重复调用。

`TooltipProvider` 的 `delay_duration` 只写入 provider data attribute；当前
关闭 grace period 的 `on_pointer_leave` 使用 150ms，不能把 provider attribute
误读成这个 timer 的实际覆盖值。

## 主题

`ui::theme` 使用 `theme!` 生成 `ShadcnTheme`，字段包括 background、foreground、
primary、secondary、muted、accent、destructive、border、input、ring 和 radius。
`shadcn_light_theme()` 与 `shadcn_dark_theme()` 提供默认 token，`inject_shadcn_base_styles()`
负责基础 CSS：

```rust
let light = silex::ui::shadcn_light_theme();
let dark = silex::ui::shadcn_dark_theme();
let _ = (light, dark);
silex::ui::inject_shadcn_base_styles();
```

主题值本身是 `silex_css` 的类型化值；要做响应式局部 patch，应使用 CSS crate
的主题 API，而不是修改 UI 组件内部生成的 class 字符串。

## 生命周期、可访问性和限制

- UI 组件通过 `data-slot`、`data-state`、`data-value`、`data-orientation` 等
  attribute 暴露结构和状态；这些标记是样式/测试钩子，不是独立状态存储。
- `Dialog`、`Popover`、`Tooltip` 的 overlay/content 可能通过 Portal 挂到
  `document.body`，其 cleanup 由 owner 负责；重复打开/关闭应检查没有重复
  content。
- API 默认值由 `Signal` promotion 产生，可能返回 `SilexError`；不要在组件
  factory 里用 `unwrap` 掩盖 runtime mismatch 或 owner close。
- 组件使用的 Tailwind class 需要对应扫描/生成配置；Rust 编译通过不代表页面
  CSS bundle 已经包含所有 class。
- 这里的 shadcn 组件是 Silex 的 view 组合实现，不承诺与 React/Radix 版本的
  全部 keyboard/focus 行为一致；新增交互应补充 wasm browser 测试。

## 源码与测试

- module facade：`crates/silex/src/ui.rs`
- 组件实现：`crates/silex/src/ui/`
- theme：`crates/silex/src/ui/theme.rs`
- Tailwind 与 styled 宏：`crates/silex_macros/src/`、
  [`silex_css 宏文档`](@/developer/crates/silex_css/macros/_index.md)
- facade 的 Tailwind smoke tests：`crates/silex/tests/tw_tests.rs`
- UI 的浏览器交互回归：`examples/ui/tests/browser.rs`
- Portal cleanup 回归：`crates/silex/tests/portal.rs`

如果只修改 UI 文档或 native 可编译 props，不需要执行整个 workspace；如果修改
事件、Portal、layout 或 CSS runtime 语义，应按对应底层 crate 的 browser/fallback
测试补充验证。

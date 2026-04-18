# Silex Core：响应式编程指南

`silex_core` 是 Silex 框架的心脏。它提供了一套高效、简洁且类型安全的“响应式原语”，让你能够以声明式的方式管理应用状态。Silex 的核心理念是**极致性能**与**零成本抽象**，确保你的应用在保持代码整洁的同时，拥有媲美原生 Rust 的执行效率。

本指南将带你从最基础的信号开始，逐步掌握 Silex 响应式系统的精髓。

---

## 1. 响应式的基础：信号 (Signals)

在 Silex 中，**信号 (Signal)** 是存储状态的基本单元。你可以把它想象成一个“活”的变量：当你修改它的值时，所有依赖于该值的地方都会自动收到通知并更新。

### 创建与基础操作
最常用的信号是 `RwSignal`（可读写信号）。

```rust
use silex::prelude::*; // 推荐使用 prelude 导入核心工具

// 1. 创建一个操作对：只读信号与写入器
let (count, set_count) = Signal::pair(0); 

// 或者通过 RwSignal 直接创建（包含读写能力，最为常用）
let count = RwSignal::new(0);

// 2. 读取值 (响应式)
println!("当前值: {}", count.get());

// 3. 修改值
count.set(10);

// 4. 就地更新 (推荐用于结构体，避免频繁克隆)
count.update(|n| *n += 1);
```

> [!TIP]
> **信号是 `Copy` 的**：在 Silex 中，所有的信号句柄（如 `RwSignal`, `ReadSignal`, `Signal`）都实现了 `Copy` 特征。这意味着你可以像传递整数一样在组件间自由传递它们，**无需**使用 `.clone()`。

---

## 2. 自动化的力量：派生计算

响应式最强大的地方在于，你可以基于已有信号创建“公式”。当源数据变化时，计算结果会自动更新。

### 使用运算符 (Operator Overloading)
Silex 为信号重载了算术和逻辑运算符，让代码读起来就像普通的 Rust 代码。

```rust
let (a, _) = Signal::pair(10);
let (b, _) = Signal::pair(20);

// sum 是一个派生信号，当 a 或 b 变化时，它会自动重算
let sum = a + b; 
let is_positive = sum.greater_than(0); // 也可以使用流畅化 API
```

### 使用 `rx!` 宏：智能与性能的平衡
对于更复杂的逻辑，推荐使用 `rx!` 宏。它能自动追踪闭包内使用的所有信号，并提供极致性能。

#### **`$变量` 语法：零拷贝访问**
在 `rx!` 中访问信号时，使用 `$` 前缀可以直接获取内部引用的视图，完全避免数据克隆。

```rust
let first_name = RwSignal::new("Alice".to_string());
let last_name = RwSignal::new("Smith".to_string());

// $first_name 实际上是 &String 类型。
// 宏会自动展开为嵌套引用访问，实现真正的零拷贝。
let full_name = rx!(format!("{} {}", $first_name, $last_name));
```

#### **极致优化：`@fn` 静态分发**
如果你确信表达式中**不捕获**局部外部变量（仅使用 `$信号` 和全局/常量），可以使用 `@fn` 前缀，这能显著减少编译后的代码体积。

```rust
// 零堆内存分配模式：将计算转化为极其轻量级的静态函数调用
let display = rx!(@fn if *$count > 0 { "Visible" } else { "Hidden" });
```

---

## 3. 极致性能：读取路径的选择

为了性能最大化，Silex 区分了不同的读取方式。

### `get()` vs `read()` vs `with()`
- **`.get()`**: **【强力克隆】** 获取值的一个完整备份。仅当类型实现了 `Clone` 时可用。
- **`.read()`**: **【引用守卫】** 返回一个守卫对象，通过它可以直接读取内部数据而无需克隆，适用于大型结构体。
- **`.with(|v| ...)`**: **【闭包借用】** 将数据引用传递给闭包。这是最推荐的零拷贝读取方式。

#### **读取方式对比**
| 方法 | 性能 | 对 `Clone` 要求 | 返回类型 | 适用场景 |
| :--- | :--- | :--- | :--- | :--- |
| **`get()`** | 一般 (涉及拷贝) | 必须实现 | `T` | 简单数值 (如 `i32`, `bool`) |
| **`read()`** | 优秀 (零拷贝) | 无要求 | `RxGuard` | 需要在作用域内手动处理引用 |
| **`with()`** | 极致 (零拷贝) | 无要求 | 闭包返回值 `U` | 只需要读取结构体的某个部分 |

---

## 4. 细粒度更新：Memo 与 Slice

在大规模应用中，避免不必要的 UI 刷新是性能的关键。

### `Memo` (记忆化计算)
只有当计算结果**真正发生变化**（基于 `PartialEq`）时，`Memo` 才会通知下游。

```rust
let count = RwSignal::new(0);
// 即使 count 从 1 变到 2，is_even 依然是 false，
// 依赖 is_even 的组件不会发生无效重绘。
let is_even = count.map(|n| n % 2 == 0).memo();
```

### `SignalSlice` (响应式投影)
当你有一个庞大的全局状态，但某个组件只关心其中的一个子字段时，请使用 `.slice()`。

```rust
struct AppState { user_name: String, theme: String }
let state = RwSignal::new(AppState { ... });

// 创建一个只关注“用户名”的切片。
// 修改 theme 时，依赖 name_slice 的组件不会刷新！
let name_slice = state.slice(|s| &s.user_name);
```

---

## 5. 异步管理：Resource 与 Mutation

Silex 将异步操作（网络请求）深度集成到了响应式系统中。

### `Resource`：拉取型异步
适用于加载数据（如：拉取用户信息）。它自带 `Loading`/`Error`/`Ready` 等状态。

```rust
let user_id = RwSignal::new(1);
// 当 user_id 变化时，fetch 会自动重新执行
let user_data = Resource::new(user_id, |id| async move {
    api::fetch_user(id).await
});

// 在视图中直接使用状态映射
view! {
    Show::new(
        move || user_data.loading(),
        rx!(div("Loading...")),
        rx!(div(format!("User: {:?}", user_data.get())))
    )
}
```

### `Mutation`：触发型异步
适用于提交表单、点击按钮等主动动作。

```rust
let login_action = Mutation::new(|(user, pass)| async move {
    api::login(user, pass).await
});

// 触发异步动作
login_action.mutate(("admin".into(), "password".into()));
```

---

## 6. 副作用：Effect

当你需要在信号变化时执行一些非 UI 的操作（如记录日志或手动操作原生 DOM）时，使用 `Effect`。

```rust
let count = RwSignal::new(0);

// 创建一个副作用，它会自动追踪闭包内使用的信号
Effect::new(move |_| {
    println!("计数器变了: {}", count.get());
});
```

---

## 总结：Silex Core 的优势

1.  **极简 API**：通过宏和运算符重载，让响应式代码读起来像原生 Rust。
2.  **极致小巧**：内部采用“响应式归一化”技术，极大减少了泛型单态化导致的编译体积膨胀。
3.  **极致流畅**：基于 `RxGuard` 的自适应读取系统，确保了应用在处理大数据时依然保持零拷贝的高性能。

掌握了 `silex_core` 的这些原语，你就已经拥有了构建高性能复杂前端应用的核心武器。下一步，可以查阅 [Silex DOM](./silex_dom/README.md) 了解如何将逻辑渲染到浏览器中。

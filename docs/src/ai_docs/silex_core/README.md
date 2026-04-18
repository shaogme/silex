# Crate: `silex_core`

**Silex 应用开发的核心响应式 API 层。**

此 Crate 对底层的 `silex_reactivity` 进行了高层级封装。其核心目标是通过**响应式归一化 (normalization)**、**Rx 委托 (Rx Delegate)** 和**自适应读取 (Adaptive Read)** 技术，在保证零拷贝 (Zero-copy) 性能的同时，极大地缩减因 Rust 泛型单态化导致的编译体积。

---

## 核心架构：类型擦除与 Rx 委托

Silex 采用了一种独特的“委托与擦除”架构来平衡易用性与性能：

1.  **Rx 包装器 (`Rx<T, M>`)**: 用户面对的统一接口层。它持有逻辑负载并通过宏和 Trait 为各种类型（常量、信号、闭包、元组）提供一致的开发体验。**关键优化**：其内部闭包现在通过 `Box<dyn Fn() -> T>` 进行类型擦除并存储在池中，确保相同返回类型的不同闭包共享同一个 `Rx<T>`，极大缩减了单态化膨胀。
2.  **响应式规范化 (Signal Canonicalization)**: 通过 `into_signal()` 方法，系统将各种实现背景的 `Rx<T>`（跨池化闭包、算子组合、常量等）统一规范化为轻量级的 `Signal<T>` 句柄。由于 `Signal<T>` 满足 `Copy` 且屏蔽了具体的实现载体差异，它成为了算子 (`OpPayload`) 之间进行无缝互操作的通用“原子句柄”。
3.  **OpPayload**: 归一化的终点。它利用 **Const Generics (`const N: usize`)** 将响应式运算（加减乘除、比较、元组聚合等）转化为包含函数指针的非泛型结构体，从而切断了泛型递归。对于二元以上元组，通过 **Meta-ID (StoredValue)** 打包输入节点，使算子复杂度衡定为 `OpPayload<T, 1>`。
4.  **静态映射与常量传播 (Static Mapping & Constant Propagation)**：引入了 `StaticMapPayload` 系列（支持 1/2/3 路输入）。同时，所有响应式算子（算术、比较、映射等）在执行前均会进行**常量检测**：若所有输入均为常量，直接在编译期或初始化期计算结果并返回 `Rx::new_constant`，彻底跳过响应式节点创建。
5. **自适应读取 (Adaptive Read)**: 通过 `RxRead` 和 `RxGet` ，系统自动提供最优读取路径：
    *   **Borrowed**: 直接借用 Arena 内的数据，零拷贝 (`RxRead::read`)。
    *   **Adaptive**: 在不强制 `Clone` 约束的前提下尝试获取副本 (`RxRead::try_get_cloned`)。
    *   **Owned**: 用于强制克隆导出 (`RxGet::get`，仅针对 `Clone` 类型)。
6. **读获取严格分离 (Read & Get Separation)**: 取代了原有的自适应克隆探测，系统现将读取能力严格分解为 `RxRead`（提供引用守卫、闭包读取及**自适应克隆**，支持任何类型）和 `RxGet`（执行强力克隆导出，仅在 `Value: Clone + Sized` 时开放）。这在类型级别彻底切断了意外隐式克隆的可能性。
7. **非泛型分发器 (Non-generic Dispatcher)**：核心响应式操作（`track`、`is_disposed`、`read_to_ptr`）由 `dispatch.rs` 内的非泛型函数驱动。通过将泛型负载转化为 `NodeId` + `RxNodeKind` 枚举，系统极大程度地收拢了机器码体积，避免了在每个泛型实例中生成冗长的调度逻辑。
8. **受限的响应式投影与元组安全 (Restricted Slicing & Tuple Safety)**: `.slice()` 方法被设计为 `Signal` 和 `ReadSignal` 的特化接口。对于元组，不支持直接 `map`，必须通过 `$tuple.0` 精确分段零拷贝借用。
9. **流畅化 API (Fluent API)**: 基于 `Map`、`Memoize`、`ReactivePartialEq` 等 Trait 提供的 Blanket Implementation，为所有 `Rx` 对象注入了 `.map()`、`.map_fn()`、`.equals()`、`.greater_than()` 等原生链式调用能力。这些接口内部均优先走常量传播路径，随后走去泛型化的 `OpPayload` 或 `StaticMapPayload` 路径。

---

## 1. 核心特征系统 (Trait System)

### 1.1 数据与基础 Trait
源码路径: `silex_core/src/traits.rs`

| Trait | 定义/语义 | 备注 |
| :--- | :--- | :--- |
| `RxData` | `trait RxData: 'static` | 框架内所有响应式数据的基本约束。 |
| `RxCloneData` | `trait RxCloneData: Clone + RxData` | 满足克隆能力的数据约束，用于 `RxGet`。 |
| `RxError` | `trait RxError: Clone + Debug + RxData` | 异步资源错误类型的标准约束。 |
| `RxValue` | `type Value: ?Sized` | **系统基石**。定义节点托管的数据类型，支持 `str` 等 DST。 |
| `RxBase` | `fn track(&self)` | 提供 `id()`, `track()`, `is_disposed()`, `defined_at()`, `debug_name()`。 |

### 1.2 内部实现 Trait
源码路径: `silex_core/src/traits/read.rs`

| Trait | 关键方法 | 语义/作用 |
| :--- | :--- | :--- |
| **`RxInternal`** | `type ReadOutput<'a>` | **内部桥梁**：定义响应式读取的底层代理逻辑（Borrowed/Owned）。 |
| | `rx_read_untracked()` | 不追踪依赖，返回 `Option<ReadOutput>`。 |
| | `rx_try_with_untracked()` | 闭包式访问底层值。 |
| | `rx_is_constant()` | 探测是否为静态常量。 |
| | `rx_get_adaptive()` | **自适应回退**：无需 `Clone` 约束探测并尝试获取副本。 |

### 1.3 用户交互 Trait (API)
所有用户方法均通过 Blanket Implementation 为符合条件的类型（如 `Rx`, `Signal`, `Tuple`）提供。

#### **`RxRead` (统一读取)**
源码路径: `silex_core/src/traits/read.rs`

*   **`read() -> Output`**: 追踪依赖，返回借用/所有权守卫（销毁时 Panic）。
*   **`try_read() -> Option<Output>`**: 上述方法的非 Panic 变体。
*   **`read_untracked() -> Output`**: 不追踪依赖，返回守卫。
*   **`with(f) -> U`**: 追踪依赖，通过闭包访问。
*   **`try_get_cloned() -> Option<Value>`**: 追踪依赖，尝试获取副本（自适应，不强制 Clone）。
*   **`get_cloned_or_default() -> Value`**: 获取副本，失败则返回默认值。

#### **`RxGet` (强力克隆)**
仅当 `Value: Clone + Sized` 且 `ReadOutput` 可解引用时生效。

*   **`get() -> Value`**: 追踪依赖，克隆并返回（销毁时 Panic）。
*   **`get_untracked() -> Value`**: 不追踪依赖，克隆并返回。

#### **`RxWrite` (统一写入)**
源码路径: `silex_core/src/traits/write.rs`

*   **`update(f)`**: 就地修改并触发通知。
*   **`try_update(f) -> Option`**: 上述方法的安全变体。
*   **`set(v)`**: 覆盖值并触发通知（要求 `Sized`）。
*   **`maybe_update(f: fn -> bool)`**: 仅当闭包返回 `true` 时触发通知。
*   **`update_untracked(f)` / `set_untracked(v)`**: 静默更新（不通知订阅者）。
*   **`notify()`**: 手动发送变更更新通知。
*   **`setter(v)` / `updater(f)`**: 产生持有所有权的 `move` 闭包，用于事件回调。

#### **`IntoRx` / `IntoSignal` (归一化)**
*   **`into_rx()`**: 将任何类型转化为 `Rx<T>`。
*   **`into_signal()`**: 将各类实现背景展平为统一的枚举 `Signal<T>`。

---

## 2. 守卫机制: `RxGuard<'a, T, S>`

源码路径: `silex_core/src/traits/guards.rs`

Silex 通过 `RxGuard` 实现了透明的借用/所有权切换。

*   **`Borrowed { value: &'a T, token: Option<NodeRef> }`**:
    *   直接指向计算池 (Arena) 的稳定引用。
    *   持有 `NodeRef` token 以确保在借用存续期间节点不被重排或销毁。
*   **`Owned(S)`**:
    *   持有计算产生的临时结果（如 `map` 执行结果或内联常量）。
    *   `S` 必须实现 `GuardStorage<T>` (通常为 `S = T`)。
*   **`try_map(f)`**:
    *   **投影能力**：仅在 `Borrowed` 模式下支持将 `&T` 投射为 `&U` (子字段借用)，保持零拷贝。

---

## 3. 响应式原始组件 (Reactive Primitives)

### 3.1 `Rx<T, M>` (万能包装器)

源码路径: `silex_core/src/lib.rs`

Silex 的“智能指针”，持有 `RxInner` 变体：
*   `Constant(T)`: 静态常量。
*   `Signal(NodeId)`: 可选信号。
*   `Closure(NodeId)`: 经过池化和类型擦除的闭包计算。
*   `Op(NodeId)`: 去泛型化的运算符节点。
*   `Stored(NodeId)`: 在 StoredValue 中直接借用的外部对象。

**特化创建**:
*   `Rx::derive(Box<dyn Fn() -> T>)`: 创建响应式派生计算。
*   `Rx::derive_fn(fn() -> T)`: 零膨胀函数指针派生。
*   `Rx::effect(val)`: 创建副作用专属负载。

### 3.1 `Signal<T>` (归一化枚举)

源码路径: `silex_core/src/reactivity/signal.rs`

它是所有响应式源在逻辑上的最终形态，实现了 `Copy`, `PartialEq`, `Eq`, `Hash`。它作为“原子句柄”在框架内部流动，屏蔽了底层存储的差异。

| 变体 | 内部数据 | 描述 |
| :--- | :--- | :--- |
| `Read(ReadSignal<T>)` | `NodeId` | 后端为 `Signal::pair(v)` 的可变信号。 |
| `Derived(NodeId, ...)` | `NodeId` | 后端为池化闭包、算子或 `rx!` 产生的派生节点。 |
| `StoredConstant(NodeId, ...)` | `NodeId`| 存储在 Arena 中但不可变的常量，不触发依赖追踪。 |
| `InlineConstant(u64, ...)` | `u64` | **内联优化**：直接在枚举内存储 <= 64bit 且 `!needs_drop` 的类型，无需 Arena 分配。 |

**核心方法**:
*   `Signal::derive(f)`: 从池化闭包创建派生信号。
*   `Signal::from(value)`: 将普通值转化为 `Signal`，优先触发 `try_inline` 优化。
*   `.ensure_node_id() -> NodeId`: 确保其在 Arena 中拥有标识符。若为 `InlineConstant`，则会将其“提升”为 `StoredConstant`。
*   `.is_constant()`: 判断该信号是否为常量变体（`StoredConstant` 或 `InlineConstant`）。
*   `.slice(getter)`: 创建细粒度投影。

### 3.2 `ReadSignal<T>` / `WriteSignal<T>` / `RwSignal<T>`

源码路径: `silex_core/src/reactivity/signal/registry.rs`

这些是对 `silex_reactivity` 底层信号的原生封装，是响应式数据的源头。

*   **`ReadSignal<T>`**: 响应式只读句柄。支持 `read`, `with`, `track`。
*   **`WriteSignal<T>`**: 响应式写句柄。支持 `update`, `set`, `notify`。
*   **`RwSignal<T>`**: 读写合并句柄。通过 `Copy` 快速分发，也可通过 `.split()` 拆分。

**全局函数**:
*   **`Signal::pair(v) -> (Read, Write)`**: 创建一个新的底层响应式信号对。
*   **`untrack(f)`**: 在非追踪作用域下执行闭包（即使在该闭包内读取信号也不建立依赖）。

### 3.3 `Constant<T>` (逻辑常量)

源码路径: `silex_core/src/reactivity/signal/derived.rs`

一个轻量级的 `Rx` 负载，专门用于在 `Rx` 链中插入硬编码值而不引入任何节点开销。

### 3.4 `Memo<T>` (缓存计算)

源码路径: `silex_core/src/reactivity/memo.rs`

*   **作用**: 缓存计算结果。仅在依赖项变化且产生的新值与旧值不等（`PartialEq`）时才通知下游更新。
*   **约束**: `T: Clone + PartialEq + 'static`。
*   **内部机制**: 依赖 `silex_reactivity::memo`。支持 `.with_name()` 调试。

### 3.5 `StoredValue<T>` (非响应式存储)

源码路径: `silex_core/src/reactivity/stored_value.rs`

*   **语义**: 将非响应式数据存储在运行时 Arena 中，返回一个 `Copy` 的句柄。
*   **特点**: **不触发依赖追踪**。适用于存储复杂的内部状态或仅用于命令式调用的数据。
*   **操作**: 支持 `get_untracked`, `set_untracked` 和 `update_untracked`。

---

## 4. 运算载体与 Payload 机制

### 4.1 `OpPayload<U, const N>` (通用算子)

源码路径: `silex_core/src/reactivity/signal/ops.rs`

Silex 通过将计算逻辑“展开”至非泛型函数指针来规避膨胀：
*   **Header**: 包含 `read_to_ptr` 和 `track` 两个核心虚表指针。
*   **Trampoline (蹦床)**: 算子实现了一系列静态 `op_trampolines`，通过 `std::mem::transmute` 将存储在特定布局中的数据还原并执行计算。
*   **De-genericization**: 所有的二元运算、比较运算最终都由 `OpPayload<U, 2>` 承载。

### 4.2 `UnifiedStaticMapPayload<T>` (归一化映射)

它是 `StaticMapPayload` (1/2/3) 的统一实现，专门优化了 1 到 3 个信号映射的场景。它直接在内存中持有 `NodeId` 数组和对应的原始函数指针，实现了**零闭包、零单次单态化分配**的高频转换（如 CSS 单位转换）。

---

## 5. 细粒度操作与异步

### 5.1 `SignalSlice` (响应式投影)

源码路径: `silex_core/src/reactivity/slice.rs`

*   **接口**: `signal.slice(|v| &v.field)`。
*   **核心**: 配合 `SliceGuard` 直接投射原始引用。它持有源节点的 `NodeRef` token，确保在投影引用存续期间 Arena 不发生重排，实现真正的**零拷贝局部更新**。

### 5.2 `Resource` & `Mutation` (异步管理)

源码路径: `silex_core/src/reactivity/resource.rs`, `mutation.rs`

*   **`Resource<T, E>`**: 拉取型异步流 (Fetch)。
    *   **状态机**: `Idle -> Loading -> Ready/Error`。支持 `Reloading` (SWR) 状态。
    *   **Suspense**: 自动与 `SuspenseContext` 集成，上报异步挂起状态。
*   **`Mutation<Arg, T, E>`**: 触发型异步操作 (Submit)。
    *   **竞态检查**: 采用 **Last-in-wins** 策略，通过内部 `last_id` 自动抵消旧的异步回调。
    *   **纯净性**: 本身是 `Copy` 句柄，通过 `StoredValue` 托管执行逻辑。

### 5.3 `NodeRef<T>` & `Callback<T>` (Copy 句柄)

源码路径: `silex_core/src/node_ref.rs`, `callback.rs`

由于返回的是 `NodeId` 句柄，这些类型在 UI 树中分发时**无需 Clone**：
*   **`NodeRef<T>`**: 绑定 DOM 节点引用，用于命令式操作 (如 `.focus()`)。
*   **`Callback<T>`**: 响应式回调包装器。支持跨闭包捕获而无需显式 `clone`，通过运行时动态派发。

### 5.4 `Effect` (副作用)

源码路径: `silex_core/src/reactivity/effect.rs`

*   **`Effect::new(f)`**: 基础自动副作用。
*   **`Effect::watch(deps, callback, immediate)`**: 精确依赖观察者。仅在 `deps()` 变化且不相等时触发 `callback`。

---

## 6. 宏与内部工具

### 6.1 `rx!` (智能转换宏)

*   **机制**: 识别 `$ident` 并重写为零拷贝多路读取。宏会在闭包外部自动克隆信号句柄，并在内部执行解引用守卫读取。
*   **智能分发**:
    *   **计算模式**: 若为表达式或不带参数的闭包，生成 `Rx::derive` (池化存储)。
    *   **副作用模式**: 若闭包带有参数（如 `|el| ...`），生成 `Rx::effect` 进行存储。
*   **优化优先 (@fn)**: 使用 `rx!(@fn ...)` 路径强制进入 `StaticMapPayload` 静态分发（支持最多 3 个信号），彻底消除闭包分配与泛型膨胀。底层依赖 `macros_helper.rs` 中的静态分发助手。

### 6.2 `batch_read!` (多路读取)

*   **签名**: `batch_read!(s1, s2 => |v1: &T1, v2: &T2| { ... })`。
*   **核心**: 通过闭包嵌套实现多个信号的同步零拷贝借用。

---

## 7. 线程安全性与安全性

*   **单线程 Runtime**: 专为 WASM 优化，不支持跨线程 (`!Send / !Sync`)。
*   **Panic 安全**: 具备详细的 `DefinedAt` 追踪。
*   **内存安全**: 依托于 `RxGuard` 的借用检查和 Arena 的物理地址稳定性。

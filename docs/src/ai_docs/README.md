# AI 阅读文档编写原则 (AI Documentation Guidelines)

本目录 (`docs/src/ai_docs`) 下的文档专门为人工智能 (AI) 助手、大语言模型 (LLM) 以及自动化代码分析工具设计。
目标是建立 **Silex** 框架的**全量知识索引**，使其在进行代码生成、重构或回答问题时具备“上帝视角”。

## 核心原则 (Core Principles)

### 1. 全面覆盖 (Comprehensive Coverage)
*   **API 全集**：必须包含 Crate 下**所有**对外公开的 Struct, Enum, Trait, Function, Macro, Type Alias, Constant。
*   **内部机制**：对于关键的私有实现（如 Reactivity Runtime 的 SlotMap 管理、Signal 的类型擦除机制），必须描述其数据结构与算法逻辑，帮助 AI 理解“为什么”。
*   **文件映射**：文档应明确指出逻辑所对应的源文件路径（如 `silex_core/src/reactivity.rs`），建立文档与源码的强关联。

### 2. 接口详情 (Interface Details)
对于每一个 API 实体，必须提供：
*   **准确签名**：包含泛型 (Generics)、约束 (Bounds)、生命周期 (Lifetimes) 和修饰符 (async, unsafe, extern)。
*   **参数语义**：明确每个参数的类型、用途及合法取值范围。
*   **返回值**：明确返回类型的含义，特别是 `Option`, `Result` 的具体分支含义。
*   **类型布局**：对于 Struct/Enum，列出所有字段/变体及其类型。

### 3. 用法与含义 (Usage & Semantics)
*   **核心作用**：一句话定义该组件解决了什么问题。
*   **副作用 (Side Effects)**：明确函数的隐式行为（如：是否分配内存、是否触发 Reactivity Update、是否 Panic、是否依赖全局状态）。
*   **不变量 (Invariants)**：调用该接口前必须满足的前置条件（Pre-conditions）。
*   **线程安全**：明确标注是否实现了 `Send` / `Sync`，以及潜在的并发风险。
*   **代码示例**：提供 <Compact Mode> 的代码片段，展示调用模式。

### 4. 结构化 (Structured)
文档必须遵循严格的层级结构，便于 AI Parser 解析：
*   **H1**: Crate 名称 / 模块概览
*   **H2**: 核心概念 / 架构图解
*   **H3**: 具体 Struct / Enum / Trait
*   **H4**: 方法 / 字段详述

### 5. 高知识密度 (High Knowledge Density)
*   **拒绝废话**：禁止使用“这是一个强大的功能”、“非常有用”等主观修饰语。
*   **信息压缩**：优先使用列表、表格、伪代码和类型签名。
*   **技术精确**：使用准确的 Rust 术语 (e.g., `Interior Mutability`, `Static Dispatch`, `Drop Check`)。

### 6. 语言规范
*   **正文**：中文 (Chinese)。
*   **术语/代码**：保留英文原文 (English)，如 `Runtime`, `NodeId`, `Signal`。

## 目录结构
*   `silex_core/`: 对应 `silex_core` crate 的高密度文档。
*   `silex_reactivity/`: 对应 `silex_reactivity` crate 的高密度文档。

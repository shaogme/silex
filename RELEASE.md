# 版本发布流程 (Release Workflow)

本项目采用 **同步版本策略 (Lock-step Versioning)**。所有核心组件 (`silex`, `silex_core`, `silex_dom`, `silex_html`, `silex_macros`, `silex_reactivity`) 将始终保持相同的版本号。

## 1. 核心原则

*   **单一事实来源**：版本号在 Workspace 根目录的 `Cargo.toml` 中统一管理。
*   **同步升级**：不单独升级某个子 Crate，任何变更都将触发所有 Crate 的版本号提升。

## 2. 如何发布新版本

假设我们要从 `0.1.0-alpha.1` 升级到 `0.1.0`。

### 第一步：更新版本号

修改根目录 `Cargo.toml`：

1.  更新 `[workspace.package]` 中的 `version` 字段。
2.  更新 `[workspace.dependencies]` 中所有内部 Crate 的 `version` 字段。

```toml
# Cargo.toml

[workspace.package]
version = "0.1.0"  # <--- update this

[workspace.dependencies]
# ...
silex = { path = "silex", version = "0.1.0" }             # <--- update this
silex_core = { path = "silex_core", version = "0.1.0" }   # <--- update this
# ... (以及其他所有内部 crate)
```

> **注意**：更新 `workspace.dependencies` 是必须的，因为 Cargo 发布到 crates.io 时需要确切的版本号，不能仅依赖 `path`。

### 第二步：验证检查

运行完备的检查以确保依赖图正确：

```bash
cargo check --workspace
cargo test --workspace
```

### 第三步：更新文档引用 (可选)

搜索全项目中的旧版本号字符串（如 `0.1.0-alpha.1`），更新 `README.md` 或其他文档中的引用。

### 第四步：发布到 Crates.io

由于存在复杂的依赖关系（`silex` 依赖 `silex_core` 等），必须按照正确的顺序发布。我们强烈推荐使用 `cargo-release` 工具来自动化此过程。

#### 方法 A：使用 `cargo-release` (推荐)

1.  安装工具：
    ```bash
    cargo install cargo-release
    ```
2.  执行发布：
    该工具会自动计算拓扑顺序，并递归发布所有成员。
    ```bash
    # 预览
    cargo release publish --workspace --dry-run --allow-branch HEAD

    # 执行
    cargo release publish --workspace --execute --allow-branch HEAD
    ```

#### 方法 B：手动发布 (不推荐)

如果必须手动发布，请严格遵守以下顺序：

1.  `silex_reactivity`
2.  `silex_core` (依赖 reactivity)
3.  `silex_dom` (依赖 core)
4.  `silex_html` (依赖 dom)
5.  `silex_macros` (独立)
6.  `silex` (依赖以上所有)

```bash
cd silex_reactivity && cargo publish
# 等待 crates.io 索引更新...
cd ../silex_core && cargo publish
# ...
```

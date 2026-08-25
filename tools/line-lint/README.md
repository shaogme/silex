# line-lint

`line-lint` 是一个面向源码仓库的行数检查工具。它会递归发现输入路径下的
文件，根据文件名或扩展名选择 39 种语言分析器，统计有效源码行数，并通过
`list` 或 `check` 命令输出结果或执行行数阈值检查。

它不是简单的 `wc -l`：默认会忽略空行、注释和测试代码，并尽量避免把字符串
中的注释标记误判为注释。每种语言都有独立的流式扫描器；Rust 也使用规则扫描
识别测试专用项，不依赖完整语法树解析。

## 快速开始

本工具是 `tools/line-lint` 下的独立 Cargo 包，要求已安装 Rust 工具链。

```bash
# 查看帮助
cargo run --manifest-path tools/line-lint/Cargo.toml -- --help

# 列出当前目录下的源码文件，按有效行数从多到少排序
cargo run --manifest-path tools/line-lint/Cargo.toml -- list .

# 只列出行数最多的 20 个文件
cargo run --manifest-path tools/line-lint/Cargo.toml -- list --limit 20 src

# 使用 4 个分析线程
cargo run --manifest-path tools/line-lint/Cargo.toml -- --jobs 4 list src

# 检查 src 下的文件是否超过 650 行
cargo run --manifest-path tools/line-lint/Cargo.toml -- check src

# 自定义上下限；超过最大值或低于最小值都会使 check 失败
cargo run --manifest-path tools/line-lint/Cargo.toml -- check \
  --max-lines 650 --min-lines 20 src
```

也可以先构建后直接调用二进制：

```bash
cargo build --release --manifest-path tools/line-lint/Cargo.toml
# 二进制位于该 Cargo 包的 target/release/line-lint
```

## 命令

### `list`

```text
line-lint list [OPTIONS] [PATH]
```

- `PATH` 默认为当前目录 `.`，可以是单个文件或目录。
- `-n, --limit N`：排序后最多输出 `N` 个报告。
- `--limit 0` 是合法调用；输入存在可报告文件时不会输出报告。
- 输出格式为 `路径: 有效行数`，例如 `src/lib.rs: 42`。
- 结果先按有效行数降序排列；行数相同时按路径升序排列。
- `list` 会输出报告，但不会根据 `max_lines` 或 `min_lines` 判定失败。

示例：

```bash
cargo run --manifest-path tools/line-lint/Cargo.toml -- list -n 10 .
```

### `check`

```text
line-lint check [OPTIONS] [PATH]
```

- `PATH` 默认为当前目录 `.`。
- `--max-lines N`：单个文件的有效行数不能大于 `N`。
- `--min-lines N`：单个文件的有效行数不能小于 `N`。
- 命令会先打印所有报告，再打印错误信息。
- 只要有一个文件违反上下限，命令就返回失败状态；错误信息会说明违反限制
  的文件数量。

默认配置为 `max_lines = 650`、不设置最小行数，因此下面两种调用等价于使用
默认最大值：

```bash
cargo run --manifest-path tools/line-lint/Cargo.toml -- check .
cargo run --manifest-path tools/line-lint/Cargo.toml -- check --max-lines 650 .
```

## 通用命令行选项

以下选项是全局选项，通常放在子命令之前；Clap 也允许全局选项出现在子命令
上下文中。

| 选项 | 作用 | 默认值 |
| --- | --- | --- |
| `--config FILE` | 显式指定 TOML 配置文件 | 自动查找 |
| `-j, --jobs N` | 请求使用的分析线程数，实际值不会超过 CPU 并行度和 64 | `8` |
| `--include-comments` | 将包含代码的注释行和纯注释行都纳入统计 | 不纳入 |
| `--include-tests` | 将测试代码纳入统计 | 不纳入 |
| `--gitignore` | 启用 `.gitignore` 规则 | 启用 |
| `--no-gitignore` | 停用 `.gitignore` 规则 | — |
| `--load-ignore` | 启用 `.ignore` 规则 | 启用 |
| `--no-ignore` | 停用 `.ignore` 规则 | — |
| `--hidden` | 启用隐藏文件和目录过滤 | 启用 |
| `--show-hidden` | 允许发现隐藏文件和目录 | — |
| `--parents` | 加载父级目录中的 ignore 规则 | 启用 |
| `--no-parents` | 不加载父级目录中的 ignore 规则 | — |

实现中还保留了一些用于配置兼容的反向/别名选项：

- `--no-ignore-comments` 是 `--include-comments` 的别名；隐藏选项
  `--ignore-comments` 强制忽略注释。
- `--no-ignore-tests` 是 `--include-tests` 的别名；隐藏选项
  `--ignore-tests` 强制忽略测试。
- `--load-gitignore` 是 `--gitignore` 的别名。
- `--use-ignore` 是 `--load-ignore` 的别名。
- `--no-hidden` 是 `--show-hidden` 的别名。
- `--load-parents` 是 `--parents` 的别名；隐藏选项 `--no-parents` 停用它。

同一组的正向和反向选项不能同时使用，例如
`--include-comments --ignore-comments` 会被命令行解析器拒绝。

## 行数统计规则

### 一行何时会被计入

对每个 UTF-8 文本文件，分析器会逐行标记代码、注释和测试状态：

1. 空行永远不计入。
2. 含有代码的行计入，即使该行末尾还有注释。
3. 纯注释行只有在 `--include-comments` 开启时才计入。
4. 默认被标记为测试代码的行不计入；使用 `--include-tests` 后恢复计入。
5. 字符串或原始字符串中的 `//`、`/*`、`#` 等文本不会按所在语言的注释
   标记处理。

行边界由源码字节预先计算：CRLF 的 `\r` 不会成为行内容，文件末尾的换行符
不会产生额外的虚拟行。文件只要包含有效代码，即使没有末尾换行也会统计最后
一行。

### 测试代码排除

测试识别由各语言模块分别实现，不是一个适用于所有语言的通用解析器。所有
语言都会先检查测试路径：路径组件为 `test`、`tests`、`__tests__`、`spec` 或
`specs` 时，整个文件会被视为测试文件；文件名主干为 `test`/`tests`、以
`_test`/`_tests` 结尾，或包含 `.test`/`.spec` 时也采用相同处理。匹配不区分
大小写。

部分语言还会识别源码中的测试声明，并按括号、大括号或语言缩进收集测试块：

| 语言 | 主要源码级识别形式 |
| --- | --- |
| Rust | `#[test]`、符合规则的 `#[cfg(...)]`；按属性和大括号范围跳过项 |
| Python | `def test_`、`async def test_`、`class Test`、`@Test`、`@ParameterizedTest`、`@pytest.mark` |
| C / C++ | `TEST(...)`、`TEST_F(...)`、`TEST_P(...)` |
| C# | `[Test...]`、`[Fact...]`、`[Theory...]` |
| Dart | `test(...)`、`group(...)` |
| Elixir | `test ...`、`test(...)` |
| Go | `func Test...` |
| Groovy | `def test...`、`@Test` |
| Java / Kotlin | `@Test`、`@ParameterizedTest` |
| JavaScript / TypeScript | `describe(...)`、`it(...)`、`test(...)`、`suite(...)`、`context(...)`、`Deno.test(...)` |
| Julia | `@test`、`@testset` |
| Perl | `sub test_...`、`sub test...` |
| PHP | `it(...)`、`test(...)`、`function test...` |
| R | `test_that(...)`、`test_check(...)` |
| Ruby | `def test_...`、`it ...`、`test ...` |
| Scala | `test(...)`、`it(...)`、`@Test` |
| Swift | `func test...` |
| Zig | `test ...` |

Rust 使用基于属性和大括号的流式容错扫描，不会调用 `syn` 或其他 AST 解析器。
`#[test]` 会标记后续项；包含 `test` 的 `#[cfg(...)]` 也可能被标记，但
`#[cfg(not(test))]` 和同时包含 `any`、`feature` 的条件会被保留。例如
`#[cfg(any(test, feature = "x"))]` 不会仅凭包含 `test` 就排除该项。

这些规则是源码级启发式识别，并不等同于完整的测试框架解析。未被当前语言
模块识别的测试写法可能仍会计入；如果需要绝对可预测的结果，可以使用测试
目录/文件名约定或显式使用 `--include-tests` 对照检查。

## 文件发现与跳过规则

输入可以是文件或目录。输入为目录时，工具递归遍历普通文件，并执行以下处理：

- 默认读取 `.ignore` 和 `.gitignore`，且会加载父级目录中的相关规则。
- 默认排除隐藏文件和隐藏目录；使用 `--show-hidden` 可关闭该过滤。
- 始终跳过路径中的 `.git` 目录。
- 不跟随符号链接。
- 不读取 Git 全局 ignore 和 `.git/info/exclude` 规则。
- 文件发现按路径排序；之后由分析线程并行处理。

发现阶段只负责收集文件，语言注册表会随后按文件名和扩展名选择分析器。
不支持的文件类型不会导致单个文件报错，而是被跳过；无效 UTF-8 文件同样被
跳过。文件发现按路径排序；文件分析使用并行线程，但最终报告顺序是确定的。
如果输入目录中没有任何可报告的文本文件，命令会失败并显示
`input contains no text files`。如果输入本身不是文件或目录，或路径无法读取，
命令会报告对应的文件系统错误。

## 支持的语言与文件匹配

语言和扩展名匹配均不区分 ASCII 大小写。固定文件名匹配优先于扩展名匹配。
当前注册表包含 39 个语言分析器：

| 语言 | 扩展名或固定文件名 |
| --- | --- |
| Rust | `.rs` |
| Python | `.py` |
| C | `.c` |
| C++ | `.cc`、`.cpp`、`.cxx` |
| C# | `.cs` |
| Dart | `.dart` |
| Go | `.go` |
| Groovy | `.groovy` |
| Java | `.java` |
| JavaScript | `.js`、`.jsx` |
| Kotlin | `.kt`、`.kts` |
| PHP | `.php` |
| Scala | `.scala` |
| Swift | `.swift` |
| TypeScript | `.ts`、`.tsx` |
| Zig | `.zig` |
| Elixir | `.ex`、`.exs` |
| Fish | `.fish` |
| INI | `.ini` |
| Julia | `.jl` |
| Perl | `.pl`、`.pm` |
| R | `.r` |
| Ruby | `.rb` |
| Shell | `.sh` |
| TOML | `.toml` |
| YAML | `.yaml`、`.yml` |
| SQL | `.sql` |
| Haskell | `.hs`、`.lhs` |
| CSS | `.css` |
| Less | `.less` |
| SCSS | `.scss` |
| Vue | `.vue` |
| HTML | `.html`、`.htm` |
| XML | `.xml` |
| SVG | `.svg` |
| Markdown | `.md`、`.markdown` |
| Dockerfile | `Dockerfile` |
| Makefile | `Makefile` |
| CMake | `CMakeLists.txt` |

对于同一目录中的 `DockerFile`、`dockerfile` 等大小写变体，固定文件名匹配仍
然有效；扩展名也使用不区分大小写的比较。

## 配置文件

### 自动发现与优先级

没有传入 `--config` 时，工具从输入路径所在目录开始，逐级向父目录查找：

1. `.line-lint.toml`
2. `line-lint.toml`

在同一目录中优先选择 `.line-lint.toml`；找到后立即停止向上查找。输入是文件
时，从该文件的父目录开始查找。传入 `--config FILE` 后只读取指定文件；如果
文件不存在或内容无法解析，会直接报错，不会回退到自动发现。

配置覆盖顺序如下，后者覆盖前者：

```text
默认值
  -> TOML 根级字段
  -> [settings]
  -> [line-lint] 或 [line_lint]
  -> 命令行选项
```

同一个 TOML 层级内不能同时设置互相矛盾的字段，例如
`ignore_comments` 和 `include_comments`，或 `ignore_tests` 和 `include_tests`。
`min_lines > max_lines` 也会使配置无效。

### 配置字段

推荐使用下表中的 snake_case 字段；实现同时接受列出的别名：

| 推荐字段 | 可用别名 | 含义 |
| --- | --- | --- |
| `max_lines` | `max-lines` | 最大有效行数 |
| `min_lines` | `min-lines` | 最小有效行数 |
| `max_file_bytes` | `max-file-bytes` | 单个文件允许读取的最大字节数 |
| `max_line_bytes` | `max-line-bytes` | 单行允许读取的最大字节数 |
| `max_source_lines` | `max-source-lines` | 单个文件允许读取的最大物理行数 |
| `rust_ast_max_bytes` | `rust-ast-max-bytes` | Rust AST 输入上限；当前流式实现仅保留并校验该字段，不执行 AST 解析 |
| `jobs` | — | 请求的并行分析线程数 |
| `ignore_comments` | `comments`、`ignore-comments` | 是否忽略注释 |
| `include_comments` | `include-comments` | 是否计入注释，和 `ignore_comments` 互斥 |
| `ignore_tests` | `tests`、`ignore-tests` | 是否忽略测试 |
| `include_tests` | `include-tests` | 是否计入测试，和 `ignore_tests` 互斥 |
| `load_gitignore` | `gitignore`、`use_gitignore`、`load-gitignore` | 是否加载 `.gitignore` |
| `load_ignore` | `ignore_file`、`use_ignore`、`load-ignore` | 是否加载 `.ignore` |
| `hide_hidden` | `hidden`、`hidden_files` | 是否隐藏隐藏文件 |
| `load_parents` | `parents`、`parent_ignore`、`load-parents` | 是否加载父级 ignore 规则 |

根级和嵌套配置可以混用。例如：

```toml
# 根级配置
max_lines = 650
ignore_comments = true
ignore_tests = true

[settings]
min_lines = 20
load_gitignore = true
load_ignore = true
jobs = 4

[line_lint]
hide_hidden = true
load_parents = true
max_file_bytes = 67108864
max_line_bytes = 1048576
max_source_lines = 1000000
```

如果希望使用带连字符的表名，也可以写成：

```toml
[line-lint]
include_comments = true
```

命令行选项始终覆盖 TOML。例如：

```bash
cargo run --manifest-path tools/line-lint/Cargo.toml -- \
  --config line-lint.toml --include-comments check .
```

`max_file_bytes`、`max_line_bytes`、`max_source_lines` 和
`rust_ast_max_bytes` 目前只能通过 TOML 配置；命令行提供 `--jobs`、
`--max-lines` 和 `--min-lines` 覆盖。资源字段必须大于零，单行上限不能超过
文件上限，Rust 上限不能超过文件上限，`jobs` 必须在 1 到 64 之间，且
`min_lines` 不能大于 `max_lines`。

默认资源上限如下：

| 字段 | 默认值 |
| --- | ---: |
| `max_file_bytes` | `67108864`（64 MiB） |
| `max_line_bytes` | `1048576`（1 MiB） |
| `max_source_lines` | `1000000` |
| `rust_ast_max_bytes` | `8388608`（8 MiB；当前不执行 AST 解析） |
| `jobs` | `8` |

## 处理流程与内部结构

工具的核心流程如下：

```text
CLI 参数
  -> ConfigLoader / LintSettings
  -> FileCollector / FileSet
  -> LanguageRegistry / LanguageAnalyzer
  -> FileAnalyzer / FileReport
  -> ReportSorter 或 CheckPolicy
```

主要模块职责：

- `src/main.rs`：解析 CLI，执行 `list`/`check`，打印报告并转换退出状态。
- `src/config.rs`：默认设置、TOML 解析、配置层级覆盖、CLI 覆盖和范围校验。
- `src/files.rs`：文件发现、ignore 过滤、隐藏文件过滤、`.git` 排除和路径排序。
- `src/language.rs`：语言标识、源码文档、分析器 trait 和共享报告类型。
- `src/language/registry.rs`：按固定文件名或扩展名路由到 39 个分析器。
- `src/language/*.rs`：每种语言自己的注释、字符串和测试识别规则。
- `src/line_count.rs`：读取 UTF-8 源码、调用分析器，并将不支持或无效编码的
  文件转换为可跳过结果。
- `src/lib.rs`：`LintEngine` 串联收集器和分析器，并提供报告排序函数。

文件分析通过 Rayon 并行执行；`jobs` 默认请求 8 个线程，实际线程数取请求值、
主机可用并行度和 64 的较小值。分析器是静态、无共享可变状态的实现，结果会
按路径稳定排序。工具输出的是每个文件的有效行数，不会修改源文件，也不会
生成报告文件。

完整报告最多保存 100,000 个文件。`list --limit N` 使用有界的 Top-N 存储，
`N` 不能超过 100,000；不带 `--limit` 的完整报告也受相同上限保护。

## 错误与退出状态

- 成功完成 `list` 或没有违反限制的 `check` 返回成功状态。
- `check` 会先输出已生成的报告；发现超限/低于下限文件后返回失败状态，并将
  违反数量写入标准错误。
- 配置错误、输入路径错误、读取错误、资源上限错误、语言分析错误，以及没有
  可报告文本文件时返回失败状态，并将诊断信息写入标准错误。
- 不支持的语言和无效 UTF-8 文件在单文件分析阶段会被跳过，而不是单独使整个
  任务失败；但如果跳过后没有任何报告，整个任务仍会失败。
- `min_lines` 大于 `max_lines` 时，配置加载失败。

## 开发与验证

在 `tools/line-lint` 目录执行：

```bash
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

测试覆盖配置默认值和覆盖层、ignore 文件发现、语言注册、CRLF 行边界、注释
与字符串处理、Rust 测试项识别，以及 CLI 的排序、限制和配置行为。

## 已知边界

- 行数统计是语言感知的词法/规则扫描，不是所有语言的完整编译器解析；复杂
  的宏、预处理器、嵌入式语言或不常见测试框架写法可能需要对应模块补充规则。
- 未知扩展名不会按普通文本统计，因此像没有扩展名的普通 README 文件默认会
  被跳过。
- 测试识别以当前源代码规则为准，目录和文件名命中测试约定时会整体排除文件；
  请避免把生产代码目录命名为 `test`、`spec` 等测试目录名。

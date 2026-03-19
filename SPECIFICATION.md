# Alloy-Check Specification (Alloy Rust 代码规范)

本项目致力于通过自动化工具 `alloy-check` 强制执行一套严格、统一且高质量的 Rust Workspace 代码规范。本规范是工具检测的唯一事实标准。

## 1. 编译与静态分析 (Build & Analysis)

### 1.1 禁止任何 Warnings
代码合并或交付前，必须通过以下检查且无任何警示信息：
- 执行 `cargo check --all-targets -- -D warnings` 无 warnings（可选追加 `--features <F>` 或 `--all-features`）。
- 执行 `cargo clippy --all-targets -- -D warnings` 无 warnings（可选追加 feature 参数）。
- *注：严禁使用 `#[allow(...)]` 规避通用规范，除非是在极端特殊情况下并附带充分理由的注释。*

### 1.2 检测代码格式化
- 必须运行 `cargo fmt --all --check` 以检测代码是否符合格式化规范。
- 严禁修改默认的 `rustfmt.toml` 配置，除非经过团队一致同意。

### 1.3 禁止特定 `#[allow(...)]` 压制项

以下 Clippy lint **严禁**通过 `#[allow(...)]` 进行压制，必须修改代码从根本上解决问题：

| 禁止的 lint | 理由 | 推荐替代方案 |
|---|---|---|
| `clippy::too_many_arguments` | 参数过多（Clippy 默认阈值 7 个）表明函数职责不清 | 引入配置 struct（Builder 模式）或拆分函数 |

- **反例**：
  ```rust
  // 错误：通过 allow 掩盖设计缺陷
  #[allow(clippy::too_many_arguments)]
  pub fn process(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) { ... }
  ```
- **正例**：
  ```rust
  pub struct ProcessConfig { pub a: i32, pub b: i32, /* ... */ }
  pub fn process(cfg: ProcessConfig) { ... }
  ```

## 2. 路径与导入规范 (Paths & Imports)

### 2.1 限制全限定路径长度
除了重导出（Re-exports）的情况外，禁止在代码调用中使用过长的全限定命名空间路径。
- **长度计算方法**：路径长度仅计算模块路径前缀，**不包括**末尾的项（如函数名、常量等），也不包括其所属的类型名（如结构体、枚举、Trait，通常以大写字母开头）。即：全限定路径只计算模块名和模块间的 `::` 连接符长度。
- **限制要求**：
  - 路径前缀长度禁止大于 15 字符。
  - 特例：如果路径以非预保留关键字（如 `std`, `core`）开头的外部 crate 模块路径，路径前缀长度限制放宽至 20 字符。
- **正例**：
  ```rust
  use std::collections::HashMap;
  let map: HashMap<i32, i32> = HashMap::new();
  ```
- **反例**（路径过长）：
  ```rust
  // 错误：全限定路径过长
  let map = std::collections::HashMap::<i32, i32>::new(); 
  ```

### 2.2 声明顺序与位置
- 在文件头部，声明顺序必须为：
  1. 模块文档注释 (`//! `) 和属性 (`#![...]`)。
  2. `mod` 模块声明。
  3. `use` 导入语句。
  4. 其他代码项（Struct, Enum, Function 等）。
- 内部导入：`use` 语句也可放置在具体函数的内部最顶部（如果该导入仅在函数内使用）。
- 严禁在代码块中间穿插 `use` 语句。

### 2.3 禁止使用 `mod.rs`
- 严禁在项目中使用旧版的 `mod.rs` 模式进行模块声明。
- **推荐做法**：使用与目录同名的 `.rs` 文件。例如，若有目录 `src/network/`，应使用 `src/network.rs` 而非 `src/network/mod.rs`。

## 3. 函数与逻辑设计 (Function Design)

### 3.1 禁止简单逻辑的函数别名
禁止创建参数类型、返回类型以及内部逻辑完全等价的简单函数包装（Alias）。
- **违规场景**：仅为了换个名字而调用另一个函数，且不增加任何逻辑。
- **反例**：
  ```rust
  fn foo(a: i32) -> i32 { a + 1 }
  // 错误：bar 只是 foo 的别名，没有存在意义
  fn bar(a: i32) -> i32 { foo(a) }
  ```

### 3.2 限制函数复杂度
- 单个函数体（除去注释和空白行）建议不超过 50 行。
  - 超过 75 行将触发 **Warning**。
  - 超过 100 行将触发 **Error**。
- 如果逻辑过深（嵌套层级超过 5 层），必须进行拆分。

### 3.3 限制标识符长度
- **函数名长度限制**：
  - **独立函数 (Function)**：长度超过 25 字符将触发 **Warning**，超过 30 字符将触发 **Error**。
  - **方法名 (Method)**：长度超过 20 字符将触发 **Warning**，超过 25 字符将触发 **Error**。

### 3.4 文件长度限制
- 单个 `.rs` 源文件的总行数（包括代码、注释和空行）不得超过 800 行。
  - 超过 650 行将触发 **Warning**。
  - 超过 800 行将触发 **Error**。

### 3.5 禁止非 Trait impl 的空 `impl` 块
- 在非 `impl Trait for Type` 形式的普通 `impl` 块中，禁止出现空块（body 内无任何 item）。
- **理由**：空的 `impl Struct {}` 没有任何语义价值，通常是遗留代码或误操作，应予以移除。
- **允许例外**：`impl Trait for Type {}` 形式的空实现是有意义的（如标记 Trait），允许保留。
- **反例**：
  ```rust
  struct Foo;
  impl Foo {} // 错误：空的非 Trait impl 块
  ```
- **正例**：
  ```rust
  struct Foo;
  trait Marker {}
  impl Marker for Foo {} // 正确：标记 Trait 的空实现

  impl Foo { fn bar(&self) {} } // 正确：非空 impl 块
  ```

## 4. 类型与数据设计 (Type & Data Design)

### 4.1 限制元组元素数量
元组（Tuple）的元素数量必须严格小于 4 个。
- **违规场景**：使用包含 4 个或更多元素的元组。
- **推荐方案**：使用具名结构体（Named Struct）代替过长的元组，以提高代码可读性和可维护性。
- **反例**：
  ```rust
  let data: (i32, String, bool, f64) = (1, "name".into(), true, 3.14); // 错误：元素数量为 4
  ```

## 5. 错误处理与安全性 (Error Handling & Safety)

### 5.1 禁止非预期的 Panic
- 在非测试（Test）代码中，严禁直接使用 `panic!`、`unwrap()` 或 `expect()`。
- **替代方案**：统一使用 `Result<T, E>` 或 `Option<T>` 进行错误传递，并使用 `?` 操作符处理。

### 5.2 显式 Safe Code
- 除非底层性能优化或外部 FFI 调用，否则严禁使用 `unsafe` 块。
- 使用 `unsafe` 时，必须在其上方提供安全性理由：
  - **API 级 Unsafe (如 `unsafe fn` 定义、`unsafe trait` 定义)**：必须通过 `/// # Safety` 章节进行文档化（符合 Clippy 标准）。
  - **实现级 Unsafe (如 `unsafe impl` 声明、函数内 `unsafe` 块)**：必须在上方添加 `// SAFETY:` 注释说明。如果 `unsafe` 是语句的主值（如在 `let` 绑定或赋值中），注释应置于该语句上方。

## 6. 文档与元数据 (Documentation & Metadata)

### 6.1 公有接口文档
- 所有声明为 `pub` 的 struct, enum, function, trait 必须包含 `///` 文档注释。
- 文档应包含：功能描述、参数说明（如果非显而易见）以及可能的错误情况（Panics/Errors）。

### 6.2 Workspace 元数据
- 每个成员 Crate 的 `Cargo.toml` 必须包含 `description`、`license` (建议 MIT/Apache-2.0) 和 `edition = "2024"`。
- edition 必须大于或等于 2024

---

## 7. 工具行为 (Tooling Behavior)

`alloy-check` 工具将按照以下逻辑运行：
1. **输入**：Rust Workspace 根目录。
2. **过程**：
   - 调度 `cargo` 原生命令进行基础检查。
   - 解析 AST（抽象语法树）进行自定义规则（如上述 2.1, 3.1 等）的深度扫描。
3. **输出**：
   - 默认以人类可读格式输出错误列表，输出目标默认为标准输出 (stdout)。
   - 工具支持 `--output <PATH>` 或 `-o <PATH>` 参数，将输出结果写入到指定的本地文件。当写入文件时，工具会自动禁用颜色代码。
   - 工具支持 `--format ron` 参数以 RON (Rusty Object Notation) 格式输出。输出结构定义如下：
     ```rust
     pub struct Report {
         pub diagnostics: Vec<Diagnostic>,
     }

     pub struct Diagnostic {
         pub file: PathBuf,
         pub line: usize,
         pub column: usize,
         pub message: String,
         pub severity: Severity,
         pub code: String,
         pub suggestion: Option<String>,
     }

     pub enum Severity {
         Error,
         Warning,
     }
     ```

4. **两种运行模式**：

   | 模式 | 触发条件 | `cargo check` / `cargo clippy` 行为 |
   |---|---|---|
   | **Human 模式**（默认） | 未指定 `--format ron` | 执行 `RUSTFLAGS="-D warnings" cargo check --all-targets` 和 `cargo clippy --all-targets -- -D warnings`，原始输出**完整透传**到终端；若命令失败则**立即以错误码退出**，不再执行后续步骤（元数据检查、AST 分析等）。<br>*注：`cargo check` 不支持 `-- <rustc-flag>` 语法，故通过 `RUSTFLAGS` 环境变量传递。* |
   | **JSON 模式** | `--format ron` | 执行带 `--message-format=json` 的命令，解析 JSON 输出并写入 `report`，后续步骤照常运行。 |

   - **features 参数**：`--features <FEATURE_LIST>` 和 `--all-features` 为可选参数，按需传入；两者互斥，`--all-features` 优先。
   - 如果违反任何规范且达到 **Error** 严重度，工具必须以非零状态码 (Exit Code != 0) 退出。
   - 如果仅存在 **Warning** 严重度的问题，工具应以零状态码 (Exit Code = 0) 退出。
   - 打印详细的错误列表，指明错误类型、所在文件、行号及修复建议。

---

## 8. 排除规则 (Exclusions)

在某些特殊情况下，可以排除特定的文件或目录：
- 默认排除 `target/` 目录。
- 自动化生成的代码（如 `prost` 生成的 protobuf 代码）应通过文件名后缀（如 `.rs` 结尾但包含 `generated`）或配置进行排除。
- 在 `Cargo.toml` 中配置 `[package.metadata.alloy-check.ignore]` 列表。

## 9. 诊断错误码 (Diagnostic Codes)

`alloy-check` 定义了以下唯一的错误码进行问题的诊断与汇报：

- **PATH001**: 全限定路径前缀过长。
- **PATH002**: 文件头部或代码块中的 `use`、`mod` 声明顺序不正确。
- **PATH003**: 禁止使用 `mod.rs`（遗留的模块系统）。
- **FUNC001**: 函数体过长（具有 50行/75行/100行 三级严重度）。
- **FUNC002**: 函数嵌套层级过深（嵌套大于 5 层）。
- **FUNC003**: 存在简单逻辑的函数包装/别名。
- **IMPL001**: 存在非 `impl Trait for Type` 形式的空 `impl` 块。
- **TYPE001**: 元组元素数量过多（不得超过 3 个）。
- **SAFE001**: 在非测试代码中使用了 `unwrap()` 或 `expect()`。
- **SAFE002**: 在非测试代码中调用了 `panic!`、`core::panic!` 等引发非预期恐慌的宏。
- **SAFE003**: 代码中含 `unsafe` 块或声明，但未提供充足说明。API 定义需 `/// # Safety` 文档章节，实现/逻辑块需 `// SAFETY:` 注释。
- **SAFE004**: `// SAFETY:` 注释位置不当。如果 `unsafe` 块是语句（如 `let`、赋值、`const` 或 `static`）的主值，注释必须放在该语句之前，严禁夹在语句中间。
- **DOC001**: 声明为 `pub` 的接口缺少 Rustdoc (`///` 或 `#[doc]`) 或潜在的文档生成宏。
- **ID001**: 标识符（如函数名、变量名）长度过长。
- **FILE001**: 单个源文件总行数超过 800 行。
- **META001**: `Cargo.toml` 中 `edition` 未按要求配置。
- **META002**: `Cargo.toml` 中缺少有意义的 `description`。
- **META003**: `Cargo.toml` 中缺少有意义的 `license`。
- **LINT001**: 在 `#[allow(...)]` 中出现了被明确禁止的 lint（如 `clippy::too_many_arguments`）。

---

**Specification Version**: 1.0.0
**Last Updated**: 2026-03-16
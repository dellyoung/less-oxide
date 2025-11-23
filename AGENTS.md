# less-oxide AI 协作手册

> 面向未来协助本项目的 AI 代理，详细记录系统背景、模块划分、工作流、测试与性能要求、及常见扩展路径，帮助你在最短时间内进入高效贡献状态。

---

## 目录

1. [项目定位与发展方向](#项目定位与发展方向)
2. [全局架构总览](#全局架构总览)
3. [子系统详解](#子系统详解)  
   3.1 [AST 层 (`src/ast.rs`)](#ast-层-srcasrs)  
   3.2 [解析层 (`src/parser.rs`)](#解析层-srcparsers)  
   3.3 [语义求值层 (`src/evaluator.rs`)](#语义求值层-screvaluators)  
   3.4 [颜色工具 (`src/color.rs`)](#颜色工具-srccolorrs)  
   3.5 [序列化层 (`src/serializer.rs`)](#序列化层-srcserializers)  
   3.6 [公共工具 (`src/utils.rs`)](#公共工具-srcutilsrs)  
   3.7 [N-API 绑定与 JS 入口](#n-api-绑定与-js-入口)
4. [测试与基准体系](#测试与基准体系)
5. [常用命令速查](#常用命令速查)
6. [工作流与最佳实践](#工作流与最佳实践)
7. [性能与一致性要求](#性能与一致性要求)
8. [典型扩展指南](#典型扩展指南)
9. [常见问题与排查建议](#常见问题与排查建议)
10. [参考资料](#参考资料)

---

## 项目定位与发展方向

- **使命**：构建一个基于 Rust 的高性能 LESS 编译器，输出 npm 包，直接兼容现有 `less` 使用场景。
- **长期目标**：
  - 覆盖 LESS 绝大多数语言特性（变量、mixin、嵌套、运算、颜色、函数、指令等）
  - 提供稳定的 Rust API/FFI 与 Node API
  - 保持对官方 less 的高可用性替代（语义一致 + 性能更优）
  - 支持后续扩展：Source Map、插件、增量编译、WASM 等
- **当前重点**：
  - 语法与语义不断完善（mixin 默认参数、多段算术、内联颜色函数等）
  - 性能监控（Criterion + Node 对比脚本）
  - 错误信息友好（中文提示、定位信息）

---

## 全局架构总览

```text
          +-----------------------+
          |   index.js / N-API    |
          +-----------+-----------+
                      |
                  compile()
                      |
    +-----------------+-----------------------------+
    |                                               |
Parser (src/parser.rs)                Evaluator (src/evaluator.rs)
    |                                               |
    v                                               v
 AST (src/ast.rs) --(Import Resolver src/importer.rs)--> AST'
     |                                   |
     +--------------------+--------------+
                         |
                  Serializer (src/serializer.rs)
                         |
                         v
                    CSS String
```

配套模块：

- `src/color.rs`: 颜色解析与转换
- `src/utils.rs`: 序列化辅助 (缩进、压缩)
- `benches/`, `fixtures/`: 性能样本
- `scripts/benchmark.js`: Node 端基准

---

## 子系统详解

### AST 层 (`src/ast.rs`)
- **作用**：描述 LESS 文法的抽象语法树；解析器将源码映射为 AST，求值器在 AST 上做语义处理。
- **核心结构**：
  - `Stylesheet`：顶层容器，包含 `Statement` 列表。
  - `Statement`：枚举，含 `Import`、`AtRule`、`RuleSet`、`Variable`、`MixinDefinition`、`MixinCall`（顶层 mixin 调用）。
    - `ImportStatement` 保留原始文本、解析路径、是否 `css` 导入（用于 `@import (css)` 透传）。
  - `RuleSet`：选择器 + `RuleBody` 列表（声明/变量/mixin/子规则/嵌套 at-rule 等）。
  - `AtRule`：描述 `@media/@supports/@font-face/...`，保留 `name + params + body`，其中 `body` 与 `RuleBody` 复用以支持嵌套作用域。
  - `Value`/`ValuePiece`：存储值片段（字面量 + 变量引用），保留原始文本以便后续处理。
  - `MixinDefinition`/`MixinCall`：mixin 定义/调用抽象，参数支持默认值。
- **注意事项**：
  - AST 尽量保持语义信息完整，解析时不做求值。
  - 扩展语法需同步更新 AST 枚举/结构，谨慎处理 `Clone` / `Debug` 派生。

### 解析层 (`src/parser.rs`)
- **流程**：递归下降 -> 构建 AST -> 记录错误位置。
- **重点函数**：
  - `LessParser::parse`：入口，循环解析 Statement。
  - `parse_variable`、`parse_ruleset`、`parse_mixin_definition/call`：各类语法节点，`parse_mixin_definition` 支持 `when (...)` guard，`parse_mixin_call` 可在顶层语境下触发。
  - `parse_at_rule`：处理顶层或嵌套的块级 `@... {}`，保留参数字符串并继续复用 `RuleBody` 解析。
  - `parse_import`：解析 `@import ...;` 语句并保留原始文本，用于后续透传输出。
  - `read_value`：核心函数，处理值片段，支持变量插值、字符串、括号跟踪；对 `(` `)` 维护 `paren_depth` 确保函数参数安全。
- **常见扩展**：
  - 新增语法需增加相应识别函数。
  - 注意更新 `lookahead_is_*`（预判函数）以避免误判。
  - 错误提示统一通过 `LessError::parse`，保持统一格式。
- **Import 解析**：`src/importer.rs` 负责根据 `CompileOptions.current_dir/include_paths` 递归展开 `@import`，带缓存与循环检测，再把展开后的 AST 交给 Evaluator。

### 语义求值层 (`src/evaluator.rs`)
- **职责**：将 AST 解析成扁平化的 CSS 结构 (`EvaluatedStylesheet`)。
- **常见任务**：
  - 求值结果：`EvaluatedStylesheet { imports: Vec<String>, nodes: Vec<EvaluatedNode> }`，节点可为 `Rule`（扁平化 selector + declarations）或 `AtRule`（name/params + 内部声明 + 嵌套节点）。
  - 变量求值：支持作用域嵌套（`scopes` 栈）。
  - `@import`：在顶层解析为 `Statement::Import`，求值阶段直接记录到 `EvaluatedStylesheet.imports`，序列化时优先输出。
  - At-rule：`Statement::AtRule` 或 `RuleBody::AtRule` 统一由 `eval_at_rule` 处理，内部沿用 `RuleBody` 逻辑，并根据是否有父选择器决定生成嵌套 `EvaluatedNode` 还是 at-rule 自身声明。
  - mixin：`mixin_scopes` 记录定义；`expand_mixin` 处理参数匹配、默认值；为保证顺序，mixin 展开时将嵌套规则写入临时队列再合并。顶层 `Statement::MixinCall` 也会走同一套流程。
  - Detached ruleset：mixins 可接收/返回 `{ ... }` 片段，通过 `MixinArgument::Ruleset + RuleBody::DetachedCall` 结合变量存储（`VariableValue::DetachedRuleset`）来展开。
  - 属性插值：`@{var}: value;` 在 `eval_declaration` 中做字符串插值，依赖新的变量类型。
  - 算术解析：支持多段表达式、负号、乘除/加减、单位一致性检查；不支持完整运算符优先级（按出现顺序执行），未来扩展需重构解析器。
  - 颜色函数：`lighten/darken/fade/overlay` 借助 `color.rs`；`replace_inline_color_functions` 扫描字符串替换内联函数为 rgba/hex。
  - `!important`：`strip_important` 脱出多余标记。
- **潜在优化点**：
  - 运算符优先级 => 可引入简单表达式树。
  - mixin 输出顺序 => 当前策略是即刻求值，后续可考虑构建 DAG。
  - Scope 查找 => 可考虑 `HashMap`+不可变结构优化。

### 颜色工具 (`src/color.rs`)
- **提供功能**：解析十六进制/rgba 字面量、HSL/HSV 转换、格式化输出。
- `parse_color`：当前支持 `#rgb/#rrggbb/#rrggbbaa`、`rgb/rgba`。
- `lighten/darken`：转 HSL 操作；`fade` 区分 alpha；`overlay` 复用了 less 官方的颜色混合模式，便于还原 `overlay(colorA, colorB)` 调用。
- 输出函数：`format_hex` / `format_rgba`（注意保留精度，去掉尾零）。
- 若扩展颜色函数，请优先在此定义基础工具，避免 evaluator 逻辑膨胀。

### 序列化层 (`src/serializer.rs`)
- **职责**：把 `EvaluatedStylesheet` 转为最终 CSS 字符串，两种模式：
  - Pretty：递归遍历 `EvaluatedNode` 树（规则 or at-rule），带缩进、换行；使用 `utils::indent`。
  - Minified：递归压缩输出，同时用 `utils::collapse_whitespace` 去除冗余空格，确保嵌套 at-rule 结构保持。
- **扩展建议**：
  - 新增特性（如 SourceMap）需在此扩展接口。
  - 若引入媒体查询或 at-rule，请确保序列化顺序与层级正确。

### 公共工具 (`src/utils.rs`)
- 目前仅包含 `collapse_whitespace`、`indent` 等辅助函数，可在此放置通用工具。
- 注意避免引入全局状态；若需正则、缓存，请使用 `Lazy`.

### N-API 绑定与 JS 入口
- Rust 端：`src/lib.rs` 中 `#[cfg(feature = "node")]` 区块导出 `compile_less`。
  - `CompileOptions { minify, current_dir, include_paths }`：其中 `current_dir`/`include_paths` 用于解析 `@import`，Node 层可通过 `filename` 传入。
  - `LessError` 转换为 `napi::Error`，错误信息保持中文。
- Node 端：`index.js` 加载 `less_oxide.node` 或 `index.node`。
  - `scripts/quick-test.js` 用于最小化验证。
  - TS 定义 `index.d.ts` 提供类型提示（包含 `filename?: string`）。

---

## 测试与基准体系

| 类型 | 命令 | 说明 |
| --- | --- | --- |
| 单元测试 | `cargo test` | 覆盖 Rust 逻辑（parser/evaluator/lib 测试、`tests/compiler.rs` 集成用例） |
| Node 快速验证 | `npm run test` | 执行 `scripts/quick-test.js` |
| Node vs less 对比 | `npm run benchmark -- 5` | 多样例性能 + 输出一致性（`styles-base` 仅做性能对比），`--` 后参数可增减迭代次数 |
| Criterion 基准 | `cargo bench less_compile --quiet` | 统计性能报告，样本在 `fixtures/` |

运行 `npm run benchmark` 时若输出不一致，会直接抛错并终止，请优先保证一致性后再测性能。

---

## 常用命令速查

```bash
# 格式化
cargo fmt

# Rust 单测
cargo test

# 构建 napi
npm run build

# Node 快速验证
npm run test

# Node 基准与一致性
npm run benchmark -- 5

# Criterion 基准
cargo bench less_compile --quiet
```

---

## 工作流与最佳实践

1. **理解需求**：阅读 issue/需求描述，确认需修改的层级（解析/求值/序列化/绑定）。
2. **定位模块**：参考上文模块说明，锁定需改动的文件，避免跨层调逻辑。
3. **编写测试**：先写或更新对应的 Rust/JS 测试，保证覆盖新行为。
4. **实现功能**：逐步修改代码；涉及 AST/解析时留意回溯与错误提示。
5. **本地验证**：至少运行：
   ```
   cargo fmt
   cargo test
   npm run build
   npm run test
   npm run benchmark -- 5
   ```
   若修改性能路径，额外执行 `cargo bench`.
6. **编写文档**：更新 README/CONTRIBUTING/AGENTS（如有必要），保持信息同步。
7. **提交说明**：PR 或提交信息中说明变更动机、核心实现、测试/基准结果。

---

## 性能与一致性要求

- **性能目标**：相较官方 less，保持数量级整体优势（脚本中常见 7x~25x 加速）。新增逻辑不得显著拖慢常规场景；如有退化需说明并可接受。
- **一致性**：`npm run benchmark` 默认检查 baseline/mixins/arithmetic/at-rules 等样本；`styles-base` 仅用于性能回归，其余样本在 `normalizeCss` 后必须一致，有差异需在 PR 清晰描述。
- **错误处理**：保持 `LessError::ParseError/EvalError` 结构，错误信息含位置/变量名等提示；避免 panic。
- **资源管理**：避免引入全局可变状态；`Lazy` 用于编译期安全的正则与缓存。

---

## 典型扩展指南

| 目标 | 关键步骤 | 注意事项 |
| --- | --- | --- |
| 新增 LESS 运算/函数 | `parser.rs` 识别 → `evaluator.rs` 扩展 `compute_value` | 注意运算优先级、单位兼容；补充测试 + 基准 |
| 增强 mixin 功能 | 扩展 `MixinDefinition`/`MixinCall` | 小心作用域、默认参数、嵌套输出顺序 |
| 支持 @media/@supports | AST 新增节点 → parser/evaluator/serializer | 需考虑嵌套规则、序列化格式与选择器组合 |
| 暴露更多 Node API 参数 | `CompileOptions` 扩展 → `index.d.ts`、`index.js` | 确保 Rust/JS 选项同步，默认值合理 |
| 输出 Source Map | 需重构 serializer pipeline | 设计新的数据结构与序列化策略，性能影响较大 |

---

## 常见问题与排查建议

- **语法解析失败**：查看 `LessError::ParseError`，关注 position；可在 `parser.rs` 中加入日志或断点。
- **变量未定义**：检查作用域栈 `scopes` 逻辑；确认 mixin 调用顺序与变量声明时机。
- **输出顺序异常**：参考 evaluator 中 pending_nodes 的处理，确保 mixin 展开后的规则/at-rule 按预期排列。
- **颜色函数不一致**：确认 `color.rs` 的解析结果与官方 less 一致；注意 rgba 精度与四舍五入策略。
- **基准脚本差异**：使用 `normalizeCss` 比较归一化字符串；若仍差异，说明 `less` 与本实现有语义不同，需要评估是否接受或修复。

---

## 参考资料

- `README.md`：项目概述与基础使用
- `CONTRIBUTING.md`：贡献流程、规范、测试说明
- `src/AGENTS.md`：针对编译流程的更细节说明（深入解析/求值模块）
- `benches/perf.rs`、`fixtures/`：性能样本定义
- `scripts/benchmark.js`：Node 对比脚本实现
- 官方 less 文档与源码，用于对齐语义

如需深入学习，推荐按以下顺序阅读源码与文档：`src/ast.rs` → `src/parser.rs` → `src/evaluator.rs` → `src/serializer.rs` → `src/lib.rs` → `scripts/benchmark.js` → `benches/perf.rs`。

---

愿你在这里构建出更快、更强、更可靠的 LESS 编译器！🚀

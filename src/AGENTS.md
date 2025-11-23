# AI 代理内部参考手册（src 层）

> 本文聚焦 `src/` 下 Rust 模块的内部实现细节，为后续 AI 在编译管线中进行增强/修复提供操作指南。

---

## 目录

1. [代码组织与数据流](#代码组织与数据流)  
2. [AST 结构 (`ast.rs`)](#ast-结构-astrs)  
3. [解析器 (`parser.rs`)](#解析器-parsers)  
4. [求值器 (`evaluator.rs`)](#求值器-evaluators)  
5. [颜色工具 (`color.rs`)](#颜色工具-colorrs)  
6. [序列化器 (`serializer.rs`)](#序列化器-serializers)  
7. [公共工具 (`utils.rs`)](#公共工具-utilsrs)  
8. [Rust <-> Node 接口 (`lib.rs`)](#rust--node-接口-librs)  
9. [测试布局与建议](#测试布局与建议)  
10. [扩展与重构注意事项](#扩展与重构注意事项)

---

## 代码组织与数据流

```text
compile(source, options)
    ├─ Parser → AST                   (src/parser.rs + src/ast.rs)
    ├─ Import Resolver                (src/importer.rs)
    ├─ Evaluator → EvaluatedNodes     (src/evaluator.rs + src/color.rs)
    └─ Serializer → CSS String        (src/serializer.rs + src/utils.rs)
```

- `CompileOptions::{minify,current_dir,include_paths}` 传递到 Evaluator/Serializer/Importer。
- 错误通过 `LessError::{ParseError, EvalError}` 返回。
- `#[cfg(feature = "node")]` 下将 `compile_less` 暴露给 N-API。

---

## AST 结构 (`ast.rs`)

### 顶层
- `Stylesheet { statements: Vec<Statement> }`
- `Statement` 枚举：`Import(ImportStatement) / AtRule(AtRule) / RuleSet(RuleSet) / Variable(VariableDeclaration) / MixinDefinition(MixinDefinition) / MixinCall(MixinCall)`

### RuleSet & RuleBody
- `RuleSet { selectors: Vec<Selector>, body: Vec<RuleBody> }`
- `RuleBody` 枚举：`Declaration` / `NestedRule(RuleSet)` / `AtRule(AtRule)` / `Variable` / `MixinDefinition` / `MixinCall` / `DetachedCall(DetachedCall)`
- `AtRule { name: String, params: String, body: Vec<RuleBody> }`：统一覆盖 `@media/@supports/@font-face/...`，body 沿用 `RuleBody` 方便继承作用域及嵌套特性。
- 选择器使用 `Selector { value: String }` 简单封装，后续若支持复杂组合可扩展结构。

### Value 表达
- `Value { pieces: Vec<ValuePiece> }`
- `ValuePiece`：`Literal(String)` 或 `VariableRef(String)`；保留原始片段，求值阶段再处理。
- Mixin 参数使用 `MixinParam { name, default }`，default 为 `Option<Value>`。`MixinCall` 的 `args: Vec<MixinArgument>` 允许传入 `{ ... }` 规则块。

> 扩展 AST 时请注意同步 `Clone`、`Debug` 以及匹配 switch 处逻辑。

---

## 解析器 (`parser.rs`)

### Cursor
- 包含 `source: &str`，`position: usize` 等，用于逐字符读取。
- 提供 `peek_char / advance_char / expect_char / skip_whitespace_and_comments` 等工具。
- `match_str` 支持判断 `/ /`、`/* */` 注释。

### Statement 解析流程
1. `parse` 循环调用：
   - `lookahead_is_variable_decl()` -> `parse_variable`
   - `lookahead_is_import()` -> `parse_import`
   - `starts_with('@')` -> `parse_at_rule`（块级 `@... {}`）
   - `lookahead_is_mixin_definition()` -> `parse_mixin_definition`
   - `lookahead_is_mixin_call()` -> `parse_mixin_call`
   - 否则 `parse_ruleset`
2. `parse_ruleset`：
   - 读取 `{` 之前的 selector 字符串并按 `,` 分割
   - 循环处理 `RuleBody`，使用 `handle_rule_body_item`（在解析器中叫 `parse_rule_body_item`）：
     - `@` 开头 -> 变量、mixin 定义、或嵌套 at-rule
     - `.name(...)` -> mixin 定义/调用
     - `:` -> declaration
     - `{` -> nested rule
   - 确保 `}` 成对出现，错误时使用 `LessError::parse`

### `@import` 与值解析
- `parse_import`：当检测到顶层 `@import` 时，直接读取直至 `;`，记录 raw/path/is_css，交给 Import Resolver 判断是否需要内联。
- `read_value`：终止符由调用方传入（如 `;`、`}`、`,`、`)`）；维护 `paren_depth`，保证函数参数内的逗号不提前终止；处理 `'`/`"` 字符串、`\` 转义、变量 `@name`。

### At-rule
- `parse_at_rule`：读取 `@name` + 参数字符串（支持括号嵌套），遇 `{` 后复用 `parse_rule_body_item` 解析 body，并允许在 rule body 中继续出现嵌套 at-rule。

### 常见注意点
- mixin 定义和调用通过前缀 `.` 或 `#` 区分；`lookahead_is_mixin_definition` 会检查 `(...) {`
- 如果新增语法，请更新 `lookahead` 系列函数，避免误解析
- 报错信息需要包含原始位置，便于定位

### Import Resolver (`src/importer.rs`)
- 负责根据 `CompileOptions.current_dir/include_paths` 递归加载并缓存子文件，展开 `Statement::Import`，并检测循环引用。

---

## 求值器 (`evaluator.rs`)

### 作用域与数据结构
- 变量作用域：`scopes: Vec<IndexMap<String, VariableValue>>`（既可保存文本也可保存 DetachedRuleset）
- mixin 作用域：`mixin_scopes: Vec<IndexMap<String, MixinDefinition>>`
- 求值结果：`EvaluatedStylesheet { imports: Vec<String>, nodes: Vec<EvaluatedNode> }`
  - `EvaluatedNode::Rule(EvaluatedRule { selectors: Vec<String>, declarations: Vec<EvaluatedDeclaration> })`
  - `EvaluatedNode::AtRule(EvaluatedAtRule { name, params, declarations, children: Vec<EvaluatedNode> })`

### 求值顺序
1. 遍历 `Stylesheet.statements`
   - `Statement::Import` -> 记录原始语句，序列化阶段优先输出
   - `Statement::Variable` -> 直接求值保存
   - `Statement::RuleSet` -> `eval_ruleset`
   - `Statement::AtRule` -> `eval_at_rule`（无父选择器）
   - `Statement::MixinDefinition` -> 注册
   - `Statement::MixinCall` -> 直接 `expand_mixin`（selectors 为空，常用于全局 mixin 输出）
2. `eval_ruleset`：
   - 新建变量/mixin 作用域（push scope）
   - 合并父选择器（`combine_selectors` 支持 `&` 占位）
   - 遍历 `RuleBody`：
     - 变量 -> 求值存储
     - Declaration -> 计算值并存入 `declarations`
     - NestedRule -> 立即递归求值并追加到 `pending_nodes`（保持顺序）
     - AtRule -> `eval_at_rule`，并把结果 push 到 `pending_nodes`
     - MixinDefinition -> 只登记作用域
     - MixinCall -> `expand_mixin` 展开
     - DetachedCall -> 解析 `VariableValue::DetachedRuleset` 并递归求值
   - 若当前 ruleset 有声明，则写入 `EvaluatedStylesheet`
   - 将 `pending_nodes` 合并到结果中（重要：维持 mixin 展开的嵌套顺序）
   - 恢复作用域

### mixin 展开
- `expand_mixin`：
  - 查找定义 -> 校验参数个数
  - 创建新 scope，先写入传入参数，再写默认值
  - 遍历 mixin body：沿用 `handle_rule_body_item`，共享 pending_nodes
- 注意嵌套 mixin 时 scope 栈必须对称 push/pop

### 值求解
- `eval_value` -> `compute_value`
  - 先尝试 `evaluate_color_function`（纯函数形式）
  - 再执行 `replace_inline_color_functions`（字符串中内联函数）
  - 再尝试 `evaluate_arithmetic`（多段表达式）
  - 否则返回 trimmed literal
- 算术解析：用 `tokenize_expression` + `apply_operator`
  - 支持 `+`, `-`, `*`, `/`
  - 按出现顺序执行，未实现优先级
  - 限制：不同单位不能直接加减，乘法不支持两个带单位值
  - 负号与前导符号有特殊处理（`prev_was_operator`）

### 颜色处理
- 使用 `Regex` 匹配 `lighten|darken|fade`，以及解析 `overlay(colorA, colorB)`（复用了 less 官方的颜色混合逻辑）
- `parse_percentage` 支持 `%` 或 0~1 数值
- `replace_inline_color_functions` 用正则捕获内联函数并替换为十六进制/rgba 字符串

### 其他细节
- `eval_at_rule`：根据是否存在父选择器决定将声明合并到 `EvaluatedNode::Rule`（存在父选择器）或 at-rule 自身（top-level `@font-face`），并递归处理 children。
- `strip_important` 确保 `!important` 不重复
- `combine_selectors` 处理 `&` 语法
- 错误通过 `LessError::eval` 返回，信息需清晰

---

## 颜色工具 (`color.rs`)

- `parse_color`：检测 `#`/`rgb`/`rgba` 字符串，返回 `Rgba { r, g, b, a }`（0~1 浮点）
- `lighten/darken`：内部转 HSL (`rgb_to_hsl`)，分别调整亮度
- `fade`：仅修改 alpha
- `format_hex`：输出 `#rrggbb`
- `format_rgba`：输出 `rgba(r, g, b, a)`，带三位小数，自动去尾零
- 若新增颜色函数（如 `saturate`、`spin`），建议在此实现基础工具函数

---

## 序列化器 (`serializer.rs`)

- 构造 `Serializer { minify: bool }`
- `to_css` 根据 `minify` 调用 `render_pretty` 或 `render_minified`，递归遍历 `EvaluatedNode` 树，保持 at-rule 层级结构。
  - Pretty：规则与 at-rule 块缩进输出，子节点级联换行。
  - Minified：紧凑输出，声明间用 `;`，对 at-rule 参数使用 `collapse_whitespace`。
- `format_declaration`/`format_declaration_minified`：
  - 负责 `!important` 输出
  - Minified 模式下使用 `collapse_whitespace` 和去空格策略

---

## 公共工具 (`utils.rs`)

- `collapse_whitespace`：压缩连续空白为单个空格
- `indent(level)`：返回两个空格 * level 的字符串
- 如需新增纯函数工具，可以放在此文件，避免污染核心逻辑

---

## Rust <-> Node 接口 (`lib.rs`)

- `compile(source, options)`：贯穿 parser → evaluator → serializer
- 特性 `node` 下启用 `napi` 导出：
  - `JsCompileOptions { minify: Option<bool> }`
  - `#[napi] pub fn compile_less(...)`
  - 错误使用 `Error::from_reason`
- 单元测试（`#[cfg(test)]`）直接调用 `compile`
  - 覆盖变量、嵌套、mixin、算术、颜色、内联函数等
- 在增强 Node 功能时需同步更新 `index.js` 与 `index.d.ts`

---

## 测试布局与建议

- `src/lib.rs`：聚焦核心功能的单元测试
- `tests/compiler.rs`：集成测试（调用 `compile`，包含 mixin、压缩模式等）
- 新增功能时：
  - 优先在 Rust 端添加单元/集成测试
  - 若涉及 Node 层，额外在 `scripts/quick-test.js` 或新增脚本中验证
  - `fixtures/` 及 `scripts/benchmark.js` 的样例需同步更新，避免基准脚本失败

---

## 扩展与重构注意事项

1. **保持 AST 与解析一致**：新增语法需更新 `ast.rs` + `parser.rs` + `evaluator.rs` + 测试。
2. **作用域安全**：任何 push/pop scope 必须成对出现；注意 mixin 嵌套与错误早退。
3. **性能考虑**：
   - 减少重复字符串分配、正则匹配
   - 大量使用 `String::with_capacity` 预分配
   - Criterion 对比前后性能（尤其是 parser/evaluator 更改）
4. **错误信息**：保持中文提示与具体细节（变量名、位置、语法）。
5. **行为一致性**：`npm run benchmark` 会对比输出，差异需要评估是否接受。
6. **特性开关**：Node 相关代码放在 `#[cfg(feature = "node")]` 下，避免纯 Rust 构建失败。

---

祝你在 src 层改动时一切顺利，如需更多上下文，请结合项目根目录下的 `AGENTS.md` 与 `CONTRIBUTING.md` 一同阅读。🚀

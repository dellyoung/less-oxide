# 贡献指南

欢迎加入 **less-oxide** 的开发！本项目目标是提供一个可替换 `less` 官方包的高性能 LESS 编译器，因此我们非常重视代码质量、性能与使用者体验。下面整理了参与贡献的常用流程与约定。

## 先决条件

在开始之前，请确认本地环境已满足以下条件：

- 安装 Rust 稳定版工具链（推荐使用 [rustup](https://rustup.rs)）
- Node.js ≥ 18
- `npm` / `pnpm` / `yarn` 任一包管理器
- Mac / Linux / Windows 均可，但建议使用类 Unix 环境进行开发与基准测试

克隆代码后，建议执行一次依赖安装：

```bash
npm install
```

## 工作流程

1. Fork 仓库并创建本地分支，例如：
   ```bash
   git checkout -b feat/more-operators
   ```
2. 在本地完成开发、自测，确保 lint / test / bench 等步骤全部通过
3. 提交 Commit，并在 PR 中简要说明修改内容、设计思路以及测试结果
4. 等待 Code Review，如需修改请在原分支追加 commit 或进行 rebase

> **提示**：我们倾向于小步提交、信息明确的 commit，必要时请拆分多次 PR。

## 代码规范

- **格式化**：提交前请运行 `cargo fmt` 与 `npm run build`（N-API 构建会触发 `rustfmt`）
- **Rust 代码**：
  - 统一使用 2021 edition 语法
  - 避免 `unsafe`；如确有必要请在 PR 中详细解释
  - 尽量编写单元测试覆盖解析 / 求值 / 序列化的核心逻辑
- **JavaScript / TypeScript**：
  - Node 入口使用 CommonJS，保持与 npm 包兼容
  - 脚本文件优先使用现代语法（`const`/`async`），保持 `lint` 通过
- **文档**：新增功能或行为变化时，请同步更新 `README.md`、示例、基准脚本等相关文档

## 测试与验证

贡献前请至少执行以下命令：

```bash
# Rust 单元测试与集成测试
cargo test

# Criterion 基准测试（可选，耗时较长）
cargo bench less_compile --quiet

# 构建 Node 原生模块
npm run build

# Node 端快速自检
npm run test

# Node vs less 的多用例对比与性能统计
npm run benchmark -- 5      # 可调整迭代次数
```

如果你新增了重要功能，请在 `tests/` 或 `src/lib.rs` 中补足覆盖，并考虑在 `fixtures/` 增加新的基准样本。

## 性能与回归

less-oxide 将性能视为第一优先级之一。在提交包含解析 / 求值 / 序列化调整的 PR 时，请注意：

- 更新或补充 `benches/perf.rs` 与 `fixtures/*`，验证新场景
- 运行 `cargo bench` 与 `npm run benchmark`，并在 PR 描述中附上对比数据 / 趋势（如可能）
- 如出现性能回退，请提供原因分析与风险评估

## 提交信息

提交信息格式没有强制要求，但推荐使用简洁明了的英文或中文描述，例如：

```
feat(parser): 支持 mixin 默认参数语法
fix(evaluator): 修复 inline fade 函数解析顺序
docs: 补充基准脚本说明
```

最终的 Pull Request 需包含：

- 变化说明（What / Why）
- 测试情况（How to verify）
- 性能影响（如适用）

## 行为准则

我们遵循开源社区普遍认可的协作精神：

- 尊重他人的付出与时间
- 在 issue、PR 中保持友好且具体的沟通
- 对不同意见保持开放、以事实和数据支撑讨论

## 获取帮助

如遇问题，可以：

- 提交 `issue`，附上上下文、复现步骤、期望与实际结果
- 在 PR 中直接 @ 项目维护者
- 参考 `README.md`、`CONTRIBUTING.md` 与源码注释

期待你的贡献，一起让 less-oxide 更快、更强、更易用！🚀

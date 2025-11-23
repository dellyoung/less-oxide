# less-oxide

> 基于 Rust 的高性能 LESS 编译器，提供 Node.js 原生绑定，可直接替换现有 `less` 包的核心能力。

## 特性

- 🚀 使用 Rust 实现解析、求值与序列化，天然具备高性能与更好内存利用率。
- 🧩 内置变量、嵌套选择器、mixin、`@import`、算术运算、颜色函数、`!important` 等核心 LESS 特性。
- 🧱 提供 napi 原生扩展，可在 Node.js 中直接调用 `compileLess` 方法。
- 🧪 提供基础单元测试，保证核心语义的正确性，方便后续扩展。

## 性能亮点

`npm run benchmark -- 10`（Apple Silicon、本地构建）对比官方 `less` 的耗时表现：

| 用例       | 模式   | less-oxide (ms) | less (ms) | 加速比    |
| ---------- | ------ | ------------ | --------- | --------- |
| baseline   | pretty | 0.072        | 1.284     | **17.9×** |
| import     | pretty | 0.005        | 0.080     | **16.5×** |
| mixins     | pretty | 0.038        | 0.381     | **10.0×** |
| arithmetic | pretty | 0.023        | 0.198     | **8.7×**  |
| at-rules   | pretty | 0.029        | 0.340     | **11.5×** |

`less-oxide` 在大部分常见 less 场景中可达到 8×–18× 的加速。运行 `npm run benchmark -- <次数>` 可在本地复现上述结果。


## 快速开始

```sh
npm install --save less-oxide
# 或者
pnpm add less-oxide
```

在 Node.js 中使用：

```js
const { compileLess } = require('less-oxide');

const css = compileLess(`
@base-color: #ff6600;
.button {
  color: @base-color;
  &:hover {
    color: darken(@base-color, 10%);
  }
}
`);

console.log(css);
```

如需压缩输出：

```js
const css = compileLess(source, { minify: true });
```

## Rust 编译

```sh
# 构建 Node 原生模块
npm install
npm run build
```

> 构建依赖本地已安装的 Rust toolchain，如未安装请先执行 `rustup` 安装。

## 目录结构

- `src/`：核心 Rust 源码
  - `parser.rs`：LESS 语法解析
  - `evaluator.rs`：语义求值与变量/嵌套处理
  - `serializer.rs`：CSS 序列化
- `index.js`：Node.js 入口，导出 `compileLess`
- `index.d.ts`：TypeScript 类型定义
- `Cargo.toml`：Rust 构建配置
- `package.json`：npm 包元信息

## 测试

```sh
cargo test
```

`cargo` 会自动执行 Rust 端的单元测试，确保语义正确。如需编写更完整的 end-to-end 测试，可在 `scripts/` 目录新增 Node.js 脚本调用。

## 性能基准

### Rust 侧基准测试

```sh
cargo bench
```

使用 [`criterion`](https://bheisler.github.io/criterion.rs/book/index.html) 进行统计基准测试，会在 `target/criterion/` 目录下生成详细报告。

### Node.js 侧对比脚本

```sh
npm run build
npm run benchmark     # 可追加迭代次数，例如 npm run benchmark -- 500
```

脚本会对比 `less-oxide` 与官方 `less` 包在多份基准样例（基础、mixin、算术等）的编译耗时，校验两端输出一致性，并输出平均耗时与加速比。

## 许可

MIT

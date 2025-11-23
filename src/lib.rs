//! less_oxide 库入口，提供面向 Rust 与 Node.js 的 LESS 编译能力。
//! 内部主要分为三个阶段：解析（Parser）→ 语义求值（Evaluator）→ CSS 序列化（Serializer）。

mod ast;
mod color;
mod error;
mod evaluator;
mod importer;
mod parser;
mod serializer;
mod utils;

use crate::error::{LessError, LessResult};
use evaluator::Evaluator;
use importer::expand_imports;
use parser::LessParser;
use serializer::Serializer;
use std::fs;
use std::path::{Path, PathBuf};

/// LESS 编译配置，目前只提供基础开关，后续可扩展 source map、模块化等高级能力。
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// 是否输出压缩后的 CSS。
    pub minify: bool,
    /// 当前源文件所在目录，用于解析相对 @import。
    pub current_dir: Option<PathBuf>,
    /// 额外的检索目录。
    pub include_paths: Vec<PathBuf>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            minify: false,
            current_dir: None,
            include_paths: Vec::new(),
        }
    }
}

/// 编译 LESS 源码为 CSS 文本。
///
/// # 参数
/// * `source` - 待编译的 LESS 字符串
/// * `options` - 编译配置
pub fn compile(source: &str, options: CompileOptions) -> LessResult<String> {
    let parser = LessParser::new();
    let mut ast = parser.parse(source)?;
    if options.current_dir.is_some() || !options.include_paths.is_empty() {
        ast = expand_imports(
            &parser,
            ast,
            options.current_dir.as_deref(),
            &options.include_paths,
        )?;
    }

    let minify = options.minify;
    let mut evaluator = Evaluator::new(options);
    let stylesheet = evaluator.evaluate(ast)?;

    let serializer = Serializer::new(minify);
    Ok(serializer.to_css(&stylesheet))
}

/// 从文件路径编译 LESS，自动处理 @import。
pub fn compile_file<P: AsRef<Path>>(path: P, mut options: CompileOptions) -> LessResult<String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|err| LessError::eval(format!("读取文件 {} 失败: {err}", path.display())))?;
    if options.current_dir.is_none() {
        if let Some(parent) = path.parent() {
            options.current_dir = Some(parent.to_path_buf());
        }
    }
    if options.include_paths.is_empty() {
        if let Some(parent) = path.parent() {
            options.include_paths.push(parent.to_path_buf());
        }
    }
    compile(&source, options)
}

#[cfg(feature = "node")]
use napi::{Error, Result};
#[cfg(feature = "node")]
use napi_derive::napi;

/// Node.js 侧的编译选项对象。
#[cfg(feature = "node")]
#[napi(object)]
pub struct JsCompileOptions {
    /// 是否压缩输出 CSS。
    pub minify: Option<bool>,
    /// 源文件路径，用于解析 @import。
    pub filename: Option<String>,
}

/// 暴露给 Node.js 的异步编译函数。
#[cfg(feature = "node")]
#[napi]
pub fn compile_less(source: String, options: Option<JsCompileOptions>) -> Result<String> {
    let opt = options.unwrap_or(JsCompileOptions {
        minify: None,
        filename: None,
    });
    let minify = opt.minify.unwrap_or(false);
    let mut compile_options = CompileOptions {
        minify,
        ..CompileOptions::default()
    };
    if let Some(filename) = opt.filename {
        let path = PathBuf::from(&filename);
        if let Some(parent) = path.parent() {
            let dir = parent.to_path_buf();
            compile_options.current_dir = Some(dir.clone());
            compile_options.include_paths.push(dir);
        }
    }
    let result =
        compile(&source, compile_options).map_err(|err| Error::from_reason(err.to_string()))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_basic_variable() {
        let src = r"@base: #111;
body {
  color: @base;
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("color: #111"));
    }

    #[test]
    fn compile_nested_selectors() {
        let src = r".btn {
  color: #fff;
  &:hover {
    color: #000;
  }
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains(".btn:hover"));
        assert!(css.contains("color: #000"));
    }

    #[test]
    fn compile_important_flag() {
        let src = r"@base: 10px;
.box {
  margin: @base !important;
}";
        let css = compile(
            src,
            CompileOptions {
                minify: true,
                ..CompileOptions::default()
            },
        )
        .unwrap();
        assert!(css.contains("margin:10px!important"));
        assert!(!css.contains("!important!important"));
    }

    #[test]
    fn compile_mixin_invocation() {
        let src = r".rounded(@radius) {
  border-radius: @radius;
}

.card {
  .rounded(8px);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("border-radius: 8px"));
    }

    #[test]
    fn compile_arithmetic_expression() {
        let src = r"@base: 10px;
.box {
  width: @base + 5px;
  padding: (@base * 2);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("width: 15px"));
        assert!(css.contains("padding: 20px"));
    }

    #[test]
    fn compile_multiple_arithmetic_segments() {
        let src = r"@spacing: 12px;
.box {
  padding: (@spacing * 0.75) (@spacing * 1.5);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("padding: 9px 18px"));
    }

    #[test]
    fn compile_color_functions() {
        let src = r"@brand: #336699;
.btn {
  background: lighten(@brand, 20%);
  border-color: darken(@brand, 10%);
  color: fade(#ffffff, 40%);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("background: #6699cc"));
        assert!(css.contains("border-color: #264c73"));
        assert!(css.contains("color: rgba(255, 255, 255, 0.4)"));
    }

    #[test]
    fn compile_mixin_with_default() {
        let src = r".shadow(@blur: 4px) {
  box-shadow: 0 0 @blur rgba(0, 0, 0, 0.2);
}

.panel {
  .shadow();
}

.toast {
  .shadow(8px);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains(".panel"));
        assert!(css.contains("box-shadow: 0 0 4px rgba(0, 0, 0, 0.2)"));
        assert!(css.contains("box-shadow: 0 0 8px rgba(0, 0, 0, 0.2)"));
    }

    #[test]
    fn compile_color_extremes() {
        let src = r"@white: #ffffff;
.banner {
  color: fade(@white, 100%);
  background: lighten(#000, 0%);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("color: rgba(255, 255, 255, 1)"));
        assert!(css.contains("background: #000000"));
    }

    #[test]
    fn compile_arithmetic_division_and_negative() {
        let src = r"@gap: 12px;
.grid {
  margin: -(@gap / 2);
  width: (@gap * -2);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("margin: -6px"));
        assert!(css.contains("width: -24px"));
    }

    #[test]
    fn compile_inline_color_function() {
        let src = r".shadow {
  box-shadow: 0 0 5px fade(#336699, 30%);
}";
        let css = compile(src, CompileOptions::default()).unwrap();
        assert!(css.contains("rgba(51, 102, 153, 0.3)"));
        assert!(!css.contains("fade("));
    }

    #[test]
    fn compile_import_statement() {
        let src = r#"@import "reset.css";
@color: #000;
body {
  color: @color;
}"#;
        let pretty = compile(src, CompileOptions::default()).unwrap();
        assert!(pretty.trim_start().starts_with("@import \"reset.css\";"));
        assert!(pretty.contains("body {"));

        let minified = compile(
            src,
            CompileOptions {
                minify: true,
                ..CompileOptions::default()
            },
        )
        .unwrap();
        assert!(minified.starts_with("@import \"reset.css\";"));
        assert!(minified.contains("body{color:#000}"));
    }
}

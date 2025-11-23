use less_oxide::{compile, compile_file, CompileOptions};
use std::path::Path;

#[test]
fn variable_and_nesting() {
    let src = r"@spacing: 8px;
.container {
  padding: @spacing;
  .title {
    margin-bottom: @spacing;
  }
}";
    let css = compile(src, CompileOptions::default()).unwrap();
    assert!(css.contains(".container"));
    assert!(css.contains(".container .title"));
}

#[test]
fn minify_output() {
    let src = r".demo {
  color: #333;
  font-weight: bold;
}";
    let css = compile(
        src,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert_eq!(css, ".demo{color:#333;font-weight:bold}");
}

#[test]
fn mixin_and_color_functions() {
    let src = r".rounded(@radius) {
  border-radius: @radius;
}

.badge {
  .rounded(4px);
  background: lighten(#123456, 15%);
}";
    let css = compile(
        src,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert!(css.contains(".badge{border-radius:4px"));
    assert!(css.contains("background:#1f5a95"));
}

#[test]
fn mixin_default_and_override() {
    let src = r".shadow(@x: 0, @y: 2px, @blur: 4px) {
  box-shadow: @x @y @blur rgba(0, 0, 0, 0.4);
}

.dialog {
  .shadow();
}

.dialog-elevated {
  .shadow(0, 8px, 16px);
}";
    let css = compile(
        src,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert!(css.contains(".dialog{box-shadow:0 2px 4px rgba(0, 0, 0, 0.4)}"));
    assert!(css.contains(".dialog-elevated{box-shadow:0 8px 16px rgba(0, 0, 0, 0.4)}"));
}

#[test]
fn arithmetic_multiple_segments_minified() {
    let src = r"@base: 5px;
.layout {
  padding: (@base * 2) (@base * 4) (@base / 5);
}";
    let css = compile(
        src,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert!(css.contains(".layout{padding:10px 20px 1px}"));
}

#[test]
fn import_statement_passthrough() {
    let src = r#"@import (css) "https://cdn.example.com/reset.css";
body {
  color: #333;
}"#;
    let css = compile(
        src,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert!(css.starts_with(r#"@import "https://cdn.example.com/reset.css";"#));
    assert!(css.contains("body{color:#333}"));
}

#[test]
fn nested_media_queries_and_supports() {
    let src = r".panel {
  color: #333;
  @media (min-width: 800px) {
    color: #000;
    .panel__title {
      font-size: 20px;
    }
  }
}

@media (max-width: 600px) {
  .panel {
    width: 100%;
  }
}";
    let css = compile(src, CompileOptions::default()).unwrap();
    assert!(css.contains(".panel {\n  color: #333;"));
    assert!(css.contains("@media (min-width: 800px)"));
    assert!(css.contains(".panel__title"));
    assert!(css.contains("@media (max-width: 600px)"));
    assert!(css.contains(".panel {\n    width: 100%;"));
}

#[test]
fn font_face_and_keyframes_blocks() {
    let src = r"@font-face {
  font-family: 'Open Sans';
  src: url('/fonts/open-sans.woff2') format('woff2');
}

@keyframes fade-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}";
    let css = compile(
        src,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert!(css.contains(
        "@font-face{font-family:'Open Sans';src:url('/fonts/open-sans.woff2') format('woff2')}"
    ));
    assert!(css.contains("@keyframes fade-in{from{opacity:0}to{opacity:1}}"));
}

#[test]
fn compile_styles_base_fixture() {
    let path = Path::new("fixtures/styles/base.less");
    let css = compile_file(
        path,
        CompileOptions {
            minify: true,
            ..CompileOptions::default()
        },
    )
    .unwrap();
    assert!(css.contains("--trade-BG-COLOR-ACTIVE"));
    assert!(css.contains(".page{min-height:100%"));
    assert!(css.contains(".weui-btn_primary"));
}

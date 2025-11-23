use crate::ast::{Statement, Stylesheet};
use crate::error::{LessError, LessResult};
use crate::parser::LessParser;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ImportResolver<'a> {
    parser: &'a LessParser,
    include_paths: Vec<PathBuf>,
    cache: HashMap<PathBuf, Stylesheet>,
    stack: Vec<PathBuf>,
}

impl<'a> ImportResolver<'a> {
    pub fn new(parser: &'a LessParser, include_paths: &[PathBuf]) -> Self {
        Self {
            parser,
            include_paths: include_paths.to_vec(),
            cache: HashMap::new(),
            stack: Vec::new(),
        }
    }

    pub fn expand(
        &mut self,
        statements: Vec<Statement>,
        current_dir: Option<&Path>,
    ) -> LessResult<Vec<Statement>> {
        let mut result = Vec::new();
        for statement in statements {
            match statement {
                Statement::Import(import) if !import.is_css => {
                    if let Some(ref target) = import.path {
                        let resolved = self.resolve_path(target, current_dir)?;
                        if self.stack.contains(&resolved) {
                            return Err(LessError::eval(format!(
                                "检测到循环导入: {}",
                                resolved.display()
                            )));
                        }
                        self.stack.push(resolved.clone());
                        let stylesheet = self.load_stylesheet(&resolved)?;
                        let parent = resolved.parent();
                        let expanded = self.expand(stylesheet.statements, parent)?;
                        result.extend(expanded);
                        self.stack.pop();
                        continue;
                    }
                }
                other => result.push(other),
            }
        }
        Ok(result)
    }

    fn load_stylesheet(&mut self, path: &Path) -> LessResult<Stylesheet> {
        if let Some(cached) = self.cache.get(path) {
            return Ok(cached.clone());
        }
        let content = fs::read_to_string(path)
            .map_err(|err| LessError::eval(format!("读取文件 {} 失败: {err}", path.display())))?;
        let stylesheet = self
            .parser
            .parse(&content)
            .map_err(|err| Self::attach_path(err, path))?;
        self.cache.insert(path.to_path_buf(), stylesheet.clone());
        Ok(stylesheet)
    }

    fn resolve_path(&self, target: &str, current_dir: Option<&Path>) -> LessResult<PathBuf> {
        let raw = Path::new(target);
        let mut candidates = Vec::new();
        if raw.is_absolute() {
            candidates.push(raw.to_path_buf());
        } else {
            if let Some(dir) = current_dir {
                candidates.push(dir.join(raw));
            }
            for base in &self.include_paths {
                candidates.push(base.join(raw));
            }
        }
        for candidate in candidates {
            if let Some(found) = Self::find_existing(&candidate) {
                return Ok(found);
            }
        }
        Err(LessError::eval(format!("无法解析 @import 路径 {target}")))
    }

    fn find_existing(candidate: &Path) -> Option<PathBuf> {
        let mut attempts = Vec::new();
        attempts.push(candidate.to_path_buf());
        if candidate.extension().is_none() {
            attempts.push(candidate.with_extension("less"));
        }
        for attempt in attempts {
            if attempt.exists() && attempt.is_file() {
                if let Ok(real) = attempt.canonicalize() {
                    return Some(real);
                }
                return Some(attempt);
            }
        }
        None
    }
}

pub fn expand_imports(
    parser: &LessParser,
    stylesheet: Stylesheet,
    current_dir: Option<&Path>,
    include_paths: &[PathBuf],
) -> LessResult<Stylesheet> {
    let mut resolver = ImportResolver::new(parser, include_paths);
    let statements = resolver.expand(stylesheet.statements, current_dir)?;
    Ok(Stylesheet::new(statements))
}

impl<'a> ImportResolver<'a> {
    fn attach_path(err: LessError, path: &Path) -> LessError {
        match err {
            LessError::ParseError { message, position } => LessError::ParseError {
                message: format!("{message} (文件: {})", path.display()),
                position,
            },
            other => other,
        }
    }
}

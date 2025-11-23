use thiserror::Error;

/// 编译过程中统一的错误类型。
#[derive(Debug, Error)]
pub enum LessError {
    #[error("解析失败: {message} (位置 {position})")]
    ParseError { message: String, position: usize },
    #[error("语义求值失败: {0}")]
    EvalError(String),
}

pub type LessResult<T> = Result<T, LessError>;

impl LessError {
    pub fn parse<S: Into<String>>(message: S, position: usize) -> Self {
        LessError::ParseError {
            message: message.into(),
            position,
        }
    }

    pub fn eval<S: Into<String>>(message: S) -> Self {
        LessError::EvalError(message.into())
    }
}

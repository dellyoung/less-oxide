/// 压缩多余空白字符，主要用于输出压缩模式。
pub fn collapse_whitespace(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    result.trim().to_string()
}

/// 保持相对缩进的辅助函数。
pub fn indent(level: usize) -> String {
    const INDENT: &str = "  ";
    (0..level).map(|_| INDENT).collect()
}

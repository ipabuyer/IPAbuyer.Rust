//! 下载输出解析（移植自主仓库 `DownloadOutputParser`）。
//!
//! 有状态解析器：从下载命令的输出流中识别"正在请求许可"阶段（`message`
//! 等于 `purchase` 的 JSON 行）与新增的进度/日志行（按百分比去重、原文去重、
//! 剥离 ANSI 转义与控制字符）。进度正则含 lookbehind，使用 fancy-regex。

use std::sync::OnceLock;

use crate::core::json;

/// 单次解析产生的更新：是否进入请求许可阶段 + 新增日志行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutputUpdate {
    pub requesting_license: bool,
    pub log_lines: Vec<String>,
}

/// 有状态的下载输出解析器。
#[derive(Debug, Default)]
pub struct DownloadOutputParser {
    stage_buffer: String,
    log_buffer: String,
    last_logged_percent: i32,
    last_log: String,
}

impl DownloadOutputParser {
    pub fn new() -> Self {
        Self {
            stage_buffer: String::new(),
            log_buffer: String::new(),
            last_logged_percent: -1,
            last_log: String::new(),
        }
    }

    /// 处理一个输出片段。
    pub fn process_chunk(&mut self, chunk: Option<&str>) -> DownloadOutputUpdate {
        let Some(chunk) = chunk else {
            return DownloadOutputUpdate {
                requesting_license: false,
                log_lines: Vec::new(),
            };
        };
        if chunk.is_empty() {
            return DownloadOutputUpdate {
                requesting_license: false,
                log_lines: Vec::new(),
            };
        }

        let requesting_license = self.process_stage_chunk(chunk);
        let log_lines = self.process_log_chunk(chunk, false);
        DownloadOutputUpdate {
            requesting_license,
            log_lines,
        }
    }

    /// 流结束时冲刷未换行的残余缓冲。
    pub fn flush(&mut self) -> DownloadOutputUpdate {
        let requesting_license = self.process_stage_chunk("\n");
        let log_lines = self.process_log_chunk("\n", true);
        DownloadOutputUpdate {
            requesting_license,
            log_lines,
        }
    }

    fn process_stage_chunk(&mut self, chunk: &str) -> bool {
        self.stage_buffer.push_str(&sanitize(chunk));
        truncate_buffer(&mut self.stage_buffer);

        let mut requesting_license = false;
        while let Some(line) = try_read_line(&mut self.stage_buffer) {
            if let Some(token) = json::try_parse_token(line.trim()) {
                if let Some(message) = json::try_read_string(&token, &["message"]) {
                    if message.eq_ignore_ascii_case("purchase") {
                        requesting_license = true;
                    }
                }
            }
        }

        requesting_license
    }

    fn process_log_chunk(&mut self, chunk: &str, flush: bool) -> Vec<String> {
        self.log_buffer.push_str(chunk);
        truncate_buffer(&mut self.log_buffer);

        let mut lines = Vec::new();
        while let Some(line) = try_read_line(&mut self.log_buffer) {
            self.add_if_new(&line, &mut lines);
        }

        if (flush || self.log_buffer.trim().chars().count() >= 48)
            && !self.log_buffer.trim().is_empty()
        {
            let remainder = self.log_buffer.clone();
            self.add_if_new(&remainder, &mut lines);
            self.log_buffer.clear();
        }

        lines
    }

    fn add_if_new(&mut self, raw_line: &str, lines: &mut Vec<String>) {
        let line = sanitize(raw_line).trim().to_string();
        if line.is_empty() {
            return;
        }

        if let Some(progress) = try_extract_progress(&line) {
            if progress == self.last_logged_percent {
                return;
            }
            self.last_logged_percent = progress;
        }

        if self.last_log == line {
            return;
        }

        self.last_log = line.clone();
        lines.push(line);
    }
}

/// 进度正则：`(?<!\d)(\d{1,3}(?:\.\d+)?)\s*[%％]`（lookbehind 需 fancy-regex）。
fn progress_regex() -> &'static fancy_regex::Regex {
    static PROGRESS_REGEX: OnceLock<fancy_regex::Regex> = OnceLock::new();
    PROGRESS_REGEX.get_or_init(|| {
        fancy_regex::Regex::new(r"(?<!\d)(\d{1,3}(?:\.\d+)?)\s*[%％]")
            .expect("progress regex must compile")
    })
}

fn try_extract_progress(line: &str) -> Option<i32> {
    let matches: Vec<fancy_regex::Captures<'_>> = progress_regex()
        .captures_iter(line)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if let Some(last) = matches.last() {
        if let Some(value) = last.get(1) {
            if let Some(percent) = try_convert_progress(value.as_str()) {
                return Some(percent);
            }
        }
    }

    for token in json::enumerate_tokens(Some(line)) {
        if let Some(value) = json::try_read_string(
            &token,
            &[
                "progress",
                "percent",
                "percentage",
                "completed",
                "completion",
                "fraction",
            ],
        ) {
            if let Some(percent) = try_convert_progress(&value) {
                return Some(percent);
            }
        }
    }

    None
}

fn try_convert_progress(value: &str) -> Option<i32> {
    let mut number = value.trim().parse::<f64>().ok()?;
    if (0.0..=1.0).contains(&number) {
        number *= 100.0;
    }
    Some(number.round().clamp(0.0, 100.0) as i32)
}

/// 读取缓冲中的下一行（按 `\r\n`/`\r`/`\n` 消费）；无完整行返回 `None`。
fn try_read_line(buffer: &mut String) -> Option<String> {
    let index = buffer.find(['\r', '\n'])?;
    let line = buffer[..index].to_string();
    let consume = if index + 1 < buffer.len() && buffer[index..].starts_with("\r\n") {
        2
    } else {
        1
    };
    buffer.drain(..index + consume);
    Some(line)
}

/// 剥离 ANSI 转义序列与除 `\r`/`\n`/`\t` 外的控制字符。
fn sanitize(input: &str) -> String {
    static ANSI_ESCAPE_REGEX: OnceLock<regex::Regex> = OnceLock::new();
    let ansi = ANSI_ESCAPE_REGEX.get_or_init(|| {
        regex::Regex::new(r"\x1B\[[0-9;?]*[ -/]*[@-~]").expect("ansi regex must compile")
    });

    ansi.replace_all(input, "")
        .chars()
        .filter(|ch| *ch == '\r' || *ch == '\n' || *ch == '\t' || !ch.is_control())
        .collect()
}

/// 缓冲超过 4096 字符时保留末尾 4096 个字符（按字符边界截断）。
fn truncate_buffer(buffer: &mut String) {
    let char_count = buffer.chars().count();
    if char_count > 4096 {
        *buffer = buffer.chars().skip(char_count - 4096).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_chunk_empty_returns_no_update() {
        let mut parser = DownloadOutputParser::new();
        for chunk in [None, Some("")] {
            let update = parser.process_chunk(chunk);
            assert!(!update.requesting_license);
            assert!(update.log_lines.is_empty());
        }
    }

    #[test]
    fn process_chunk_detects_requesting_license_stage() {
        let mut parser = DownloadOutputParser::new();

        let update = parser.process_chunk(Some("{\"message\":\"download\"}\n"));
        assert!(!update.requesting_license);

        let update = parser.process_chunk(Some("{\"message\":\"PURCHASE\"}\n"));
        assert!(update.requesting_license);
    }

    #[test]
    fn process_chunk_deduplicates_progress_percentages() {
        let mut parser = DownloadOutputParser::new();

        let first = parser.process_chunk(Some("downloading 50%\n"));
        assert_eq!(first.log_lines, ["downloading 50%"]);

        // 同一百分比重复出现被去重。
        let second = parser.process_chunk(Some("downloading 50%\n"));
        assert!(second.log_lines.is_empty());

        let third = parser.process_chunk(Some("downloading 51%\n"));
        assert_eq!(third.log_lines, ["downloading 51%"]);
    }

    #[test]
    fn process_chunk_converts_fraction_and_json_progress() {
        let mut parser = DownloadOutputParser::new();

        let update = parser.process_chunk(Some("{\"progress\":0.5}\n"));
        assert_eq!(update.log_lines, ["{\"progress\":0.5}"]);

        let update = parser.process_chunk(Some("{\"percentage\":\"75\"}\n"));
        assert_eq!(update.log_lines, ["{\"percentage\":\"75\"}"]);
    }

    #[test]
    fn process_chunk_sanitizes_ansi_and_control_characters() {
        let mut parser = DownloadOutputParser::new();

        let update = parser.process_chunk(Some("\u{1b}[2K\u{1b}[G\u{7}progress 25%\u{1b}[0m\n"));
        assert_eq!(update.log_lines, ["progress 25%"]);
    }

    #[test]
    fn flush_emits_trailing_buffer_without_newline() {
        let mut parser = DownloadOutputParser::new();
        parser.process_chunk(Some("downloading 10%\n"));
        parser.process_chunk(Some("finalizing"));

        let update = parser.flush();
        assert_eq!(update.log_lines, ["finalizing"]);
    }

    #[test]
    fn process_chunk_buffers_partial_lines_until_complete() {
        let mut parser = DownloadOutputParser::new();

        let update = parser.process_chunk(Some("downloading 3"));
        assert!(update.log_lines.is_empty());

        let update = parser.process_chunk(Some("3%\n"));
        assert_eq!(update.log_lines, ["downloading 33%"]);
    }

    #[test]
    fn process_chunk_ignores_progress_like_numbers_inside_longer_numbers() {
        let mut parser = DownloadOutputParser::new();

        // "2023%" 不应解析出 23%（lookbehind 防御）。
        let update = parser.process_chunk(Some("build 2023%\n"));
        assert_eq!(update.log_lines, ["build 2023%"]);
    }
}

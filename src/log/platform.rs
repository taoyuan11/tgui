use super::api::LogLevel;

mod imp {
    use std::io::{self, Write};

    use super::LogLevel;

    #[cfg(test)]
    pub(super) fn format_plain_line(_level: LogLevel, _tag: &str, line: &str) -> String {
        line.trim_end_matches('\n').to_string()
    }

    pub(super) fn format_colored_line(level: LogLevel, _tag: &str, line: &str) -> String {
        let line = line.trim_end_matches('\n');
        let marker = format!("[{level}]");
        let colored_marker = format!("{}{}{}", color_code(level), marker, ANSI_RESET);
        line.replacen(&marker, &colored_marker, 1)
    }

    pub(super) fn write(level: LogLevel, tag: &str, line: &str) {
        #[cfg(test)]
        let line = format_colored_line(level, tag, line);
        #[cfg(not(test))]
        let line = format_colored_line(level, tag, line);
        let _ = writeln!(io::stderr().lock(), "{line}");
    }

    fn color_code(level: LogLevel) -> &'static str {
        match level {
            LogLevel::Trace => "\x1b[90m",
            LogLevel::Debug => "\x1b[34m",
            LogLevel::Info => "\x1b[32m",
            LogLevel::Warn => "\x1b[33m",
            LogLevel::Error => "\x1b[31m",
        }
    }

    const ANSI_RESET: &str = "\x1b[0m";
}

pub(super) fn write(level: LogLevel, tag: &str, line: &str) {
    imp::write(level, tag, line);
}

#[cfg(test)]
pub(super) fn format_plain_line(level: LogLevel, tag: &str, line: &str) -> String {
    imp::format_plain_line(level, tag, line)
}

#[cfg(test)]
pub(super) fn format_colored_line(level: LogLevel, tag: &str, line: &str) -> String {
    imp::format_colored_line(level, tag, line)
}

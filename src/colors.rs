use anstyle::{AnsiColor, Color, Style};
use std::fmt::Write as _;

/// Check if colors should be used based on environment and terminal capabilities
fn should_use_color() -> bool {
    // Check NO_COLOR environment variable
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Check if stdout is a terminal
    anstream::stderr().is_terminal() || anstream::stdout().is_terminal()
}

/// Format a target prefix [name] in grey
pub fn target_prefix(name: &str) -> String {
    grey_msg(&format!("[{}]", name))
}

/// Format a success message in cyan
pub fn success_msg(msg: &str) -> String {
    colorize(msg, AnsiColor::Cyan)
}

/// Format an error message in red
pub fn error_msg(msg: &str) -> String {
    colorize(msg, AnsiColor::Red)
}

/// Format a warning message in yellow
pub fn warn_msg(msg: &str) -> String {
    colorize(msg, AnsiColor::Yellow)
}

/// Format a message in grey (using blue for better visibility)
pub fn grey_msg(msg: &str) -> String {
    colorize(msg, AnsiColor::Blue)
}

/// Internal helper to colorize text with the given ANSI color
fn colorize(text: &str, color: AnsiColor) -> String {
    if !should_use_color() {
        return text.to_string();
    }

    let style = Style::new().fg_color(Some(Color::Ansi(color)));
    let mut buf = String::new();
    let _ = write!(buf, "{}{}{}", style.render(), text, style.render_reset());
    buf
}

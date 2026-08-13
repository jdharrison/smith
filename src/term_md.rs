//! Terminal markdown rendering for chat (and similar) output.
//!
//! Renders when stdout is a TTY and `NO_COLOR` is unset; otherwise prints raw markdown.

use std::io::{self, IsTerminal};
use termimad::{crossterm::style::Color::*, MadSkin};

fn should_render() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    io::stdout().is_terminal()
}

fn chat_skin() -> MadSkin {
    let mut skin = MadSkin::default();
    skin.set_headers_fg(Cyan);
    skin.bold.set_fg(White);
    skin.italic.set_fg(Grey);
    skin.inline_code.set_fg(Yellow);
    skin.code_block.set_fg(Yellow);
    skin.quote_mark.set_fg(DarkGrey);
    skin.bullet.set_fg(Cyan);
    skin
}

/// Print markdown to stdout (styled in a TTY).
pub fn print_markdown(md: &str) {
    let text = md.trim_end();
    if text.is_empty() {
        println!();
        return;
    }
    if !should_render() {
        println!("{text}");
        return;
    }
    chat_skin().print_text(text);
    println!();
}

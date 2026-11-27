//! Terminal output for the CLI: colors, symbols, tables and prompts.
//!
//! Written by hand on purpose. Crates such as `dialoguer` or `comfy-table`
//! would do more, but they pull in many dependencies for a few dozen lines of
//! work, and the project keeps its dependency list short.
//!
//! Everything here degrades when the output is not a terminal (a pipe, a
//! script, a test): no color codes are written and no question is asked.

use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{bail, Context, Result};

#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    Plain,
    Bold,
    Dim,
    Green,
    Red,
    Yellow,
    Cyan,
}

impl Style {
    /// The ANSI code for this style, the number between `ESC[` and `m`.
    fn code(self) -> &'static str {
        match self {
            Style::Plain => "",
            Style::Bold => "1",
            Style::Dim => "2",
            Style::Red => "31",
            Style::Green => "32",
            Style::Yellow => "33",
            Style::Cyan => "36",
        }
    }
}

/// Follows the NO_COLOR convention (https://no-color.org) and `TERM=dumb`.
fn colors_allowed() -> bool {
    std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").map_or(true, |term| term != "dumb")
}

fn paint_if(enabled: bool, text: &str, style: Style) -> String {
    if !enabled || style == Style::Plain {
        return text.to_string();
    }
    format!("\x1b[{}m{text}\x1b[0m", style.code())
}

/// Colors a text for stderr. Used by the shim, which has no `Ui`.
pub fn paint_stderr(text: &str, style: Style) -> String {
    paint_if(colors_allowed() && io::stderr().is_terminal(), text, style)
}

/// One table cell: the text and how to show it. Keeping the style apart from
/// the text lets the table measure the real width before adding color codes.
pub struct Cell {
    text: String,
    style: Style,
}

impl Cell {
    // `impl Into<String>` accepts both `&str` and `String`, so callers do not
    // have to write `.to_string()` everywhere.
    pub fn new(text: impl Into<String>) -> Self {
        Self::styled(text, Style::Plain)
    }

    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

pub struct Ui {
    color: bool,
    interactive: bool,
}

impl Ui {
    pub fn new() -> Self {
        let stdout = io::stdout().is_terminal();
        Self {
            color: colors_allowed() && stdout,
            // Questions need a person on both ends: reading and seeing.
            interactive: stdout && io::stdin().is_terminal(),
        }
    }

    /// True when we can ask questions. False in scripts, pipes and tests.
    pub fn interactive(&self) -> bool {
        self.interactive
    }

    pub fn paint(&self, text: &str, style: Style) -> String {
        paint_if(self.color, text, style)
    }

    /// A command the user can type, shown in a distinct color.
    pub fn command(&self, text: &str) -> String {
        self.paint(text, Style::Cyan)
    }

    pub fn success(&self, text: &str) {
        println!("{} {text}", self.paint("✓", Style::Green));
    }

    pub fn failure(&self, text: &str) {
        println!("{} {text}", self.paint("✗", Style::Red));
    }

    pub fn warning(&self, text: &str) {
        println!("{} {text}", self.paint("!", Style::Yellow));
    }

    /// A secondary line under a result, indented and dimmed.
    pub fn detail(&self, text: &str) {
        println!("  {}", self.paint(text, Style::Dim));
    }

    pub fn title(&self, text: &str) {
        println!("{}", self.paint(text, Style::Bold));
    }

    /// Label and value pairs, with the values aligned in one column.
    pub fn fields(&self, rows: &[(&str, String)]) {
        let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
        for (label, value) in rows {
            let label = format!("{label:<width$}");
            println!("  {}  {value}", self.paint(&label, Style::Dim));
        }
    }

    /// A list of commands with what each one does, descriptions aligned.
    pub fn commands(&self, rows: &[(&str, &str)]) {
        let width = rows
            .iter()
            .map(|(command, _)| command.len())
            .max()
            .unwrap_or(0);
        for (command, what) in rows {
            let command = format!("{command:<width$}");
            println!(
                "  {}  {}",
                self.command(&command),
                self.paint(what, Style::Dim)
            );
        }
    }

    /// A bordered table. Every row must have as many cells as `headers`.
    pub fn table(&self, headers: &[&str], rows: &[Vec<Cell>]) {
        // `chars().count()` and not `len()`: symbols like "●" take several
        // bytes but one column on screen.
        let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
        for row in rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(cell.text.chars().count());
            }
        }

        let border = |left: &str, middle: &str, right: &str| {
            let segments: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
            let line = format!("{left}{}{right}", segments.join(middle));
            println!("{}", self.paint(&line, Style::Dim));
        };
        let bar = self.paint("│", Style::Dim);
        let line = |cells: Vec<String>| {
            println!("{bar} {} {bar}", cells.join(&format!(" {bar} ")));
        };
        let pad = |text: &str, width: usize| {
            format!("{text}{}", " ".repeat(width - text.chars().count()))
        };

        border("┌", "┬", "┐");
        line(
            headers
                .iter()
                .zip(&widths)
                .map(|(header, width)| self.paint(&pad(header, *width), Style::Bold))
                .collect(),
        );
        border("├", "┼", "┤");
        for row in rows {
            line(
                row.iter()
                    .zip(&widths)
                    .map(|(cell, width)| self.paint(&pad(&cell.text, *width), cell.style))
                    .collect(),
            );
        }
        border("└", "┴", "┘");
    }

    /// A question whose answer may be empty: just Enter means "nothing".
    pub fn answer(&self, question: &str) -> Result<String> {
        self.ask(&format!("{} ", self.paint(question, Style::Bold)))
    }

    /// Prints `prompt` and reads one line. Fails when the input is closed
    /// (Ctrl-D), so a cancelled question never turns into a silent default.
    fn ask(&self, prompt: &str) -> Result<String> {
        print!("{prompt}");
        io::stdout()
            .flush()
            .context("cannot write to the terminal")?;
        let mut answer = String::new();
        let read = io::stdin()
            .lock()
            .read_line(&mut answer)
            .context("cannot read your answer")?;
        if read == 0 {
            println!();
            bail!("cancelled");
        }
        Ok(answer.trim().to_string())
    }

    /// Asks for a free text. Asks again while the answer is empty.
    pub fn input(&self, question: &str) -> Result<String> {
        loop {
            let answer = self.ask(&format!("{} ", self.paint(question, Style::Bold)))?;
            if !answer.is_empty() {
                return Ok(answer);
            }
        }
    }

    /// Yes or no. An empty answer (just Enter) takes `default`.
    pub fn confirm(&self, question: &str, default: bool) -> Result<bool> {
        let choices = if default { "Y/n" } else { "y/N" };
        loop {
            let prompt = format!(
                "{} {} ",
                self.paint(question, Style::Bold),
                self.paint(&format!("[{choices}]"), Style::Dim)
            );
            match self.ask(&prompt)?.to_lowercase().as_str() {
                "" => return Ok(default),
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => println!("Please answer y or n."),
            }
        }
    }

    /// Shows a numbered list and returns the index of the chosen option.
    /// The answer can be the number, or the exact text of a `keys` entry
    /// (for example a profile name).
    pub fn select(
        &self,
        question: &str,
        options: &[String],
        keys: &[String],
        default: Option<usize>,
    ) -> Result<usize> {
        self.title(question);
        for (index, option) in options.iter().enumerate() {
            let number = format!("{}.", index + 1);
            println!("  {} {option}", self.command(&number));
        }
        let hint = match default {
            Some(index) => format!(" [{}]", index + 1),
            None => String::new(),
        };
        loop {
            let label = if keys.is_empty() {
                "Number"
            } else {
                "Number or name"
            };
            let answer = self.ask(&format!("{label}{}: ", self.paint(&hint, Style::Dim)))?;
            if answer.is_empty() {
                if let Some(index) = default {
                    return Ok(index);
                }
            }
            if let Some(index) = keys.iter().position(|key| *key == answer) {
                return Ok(index);
            }
            match answer.parse::<usize>() {
                Ok(number) if (1..=options.len()).contains(&number) => return Ok(number - 1),
                _ => println!("Type a number between 1 and {}.", options.len()),
            }
        }
    }
}

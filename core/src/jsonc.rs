//! Changing one setting in a JSON file that may hold comments, the way the
//! editors of the VS Code family write their `settings.json`.
//!
//! `serde_json` refuses such a file, and writing it back whole would lose
//! the comments and the layout anyway. So this module only finds where the
//! top-level members are, then edits the text in one place: everything else
//! stays as it was, byte for byte.
//!
//! It reads more than strict JSON on purpose (comments, a comma after the
//! last member), because that is what those files hold. Anything it does
//! not understand is an error, and the caller then leaves the file alone.

use std::ops::Range;

use anyhow::{anyhow, bail, Result};

/// One `"key": value` of the top-level object.
struct Member {
    key: String,
    /// Where the key's opening quote is.
    start: usize,
    /// The value, as written.
    value: Range<usize>,
    /// The comma that follows the value, when there is one.
    comma: Option<usize>,
}

struct Document {
    /// Where the opening `{` is.
    open: usize,
    members: Vec<Member>,
}

impl Document {
    /// The member that counts for a key: the last one, as the editors read
    /// a key written twice.
    fn find(&self, key: &str) -> Option<usize> {
        self.members.iter().rposition(|member| member.key == key)
    }
}

/// Walks the text. `at` is a position in bytes: every character that has a
/// meaning here (`{`, `"`, `/`, `,`) is one byte long, so `at` never stops
/// in the middle of a longer character.
struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn starts_with(&self, text: &str) -> bool {
        self.bytes[self.at..].starts_with(text.as_bytes())
    }

    /// Moves past spaces, line breaks and comments.
    fn skip_blank(&mut self) -> Result<()> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => self.at += 1,
                Some(b'/') if self.starts_with("//") => {
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.at += 1;
                    }
                }
                Some(b'/') if self.starts_with("/*") => {
                    let rest = &self.bytes[self.at + 2..];
                    let end = rest
                        .windows(2)
                        .position(|pair| pair == b"*/")
                        .ok_or_else(|| anyhow!("a comment is never closed"))?;
                    self.at += 2 + end + 2;
                }
                _ => return Ok(()),
            }
        }
    }

    /// Reads a string, quotes included. `at` must be on its opening quote.
    fn string(&mut self) -> Result<Range<usize>> {
        let start = self.at;
        self.at += 1;
        loop {
            match self.peek() {
                None | Some(b'\n') => bail!("a text in quotes is never closed"),
                // A backslash protects the next character, a quote included.
                Some(b'\\') => self.at += 2,
                Some(b'"') => {
                    self.at += 1;
                    return Ok(start..self.at);
                }
                Some(_) => self.at += 1,
            }
        }
    }

    /// Reads a value of any kind without looking into it.
    fn value(&mut self) -> Result<Range<usize>> {
        let start = self.at;
        match self.peek() {
            Some(b'"') => {
                self.string()?;
            }
            Some(b'{' | b'[') => {
                // Brackets are counted; strings and comments are skipped
                // whole, so a bracket inside them does not count.
                let mut depth = 0usize;
                loop {
                    self.skip_blank()?;
                    match self.peek() {
                        None => bail!("a bracket is never closed"),
                        Some(b'"') => {
                            self.string()?;
                        }
                        Some(b'{' | b'[') => {
                            depth += 1;
                            self.at += 1;
                        }
                        Some(b'}' | b']') => {
                            depth -= 1;
                            self.at += 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Some(_) => self.at += 1,
                    }
                }
            }
            _ => {
                // A number, true, false or null: up to whatever ends it.
                while let Some(byte) = self.peek() {
                    if matches!(byte, b',' | b'}' | b']' | b'/') || byte.is_ascii_whitespace() {
                        break;
                    }
                    self.at += 1;
                }
                if self.at == start {
                    bail!("a value is missing");
                }
            }
        }
        Ok(start..self.at)
    }
}

/// Finds the top-level members. `None` for a file with no object at all
/// (empty, or comments only), which the editors read as "no setting".
fn scan(text: &str) -> Result<Option<Document>> {
    let mut scanner = Scanner {
        bytes: text.as_bytes(),
        at: 0,
    };
    // Some editors start a file with an invisible mark (a "BOM").
    if scanner.starts_with("\u{feff}") {
        scanner.at += "\u{feff}".len();
    }
    scanner.skip_blank()?;
    match scanner.peek() {
        None => return Ok(None),
        Some(b'{') => {}
        Some(_) => bail!("it does not start with `{{`"),
    }
    let open = scanner.at;
    scanner.at += 1;

    let mut members = Vec::new();
    loop {
        scanner.skip_blank()?;
        match scanner.peek() {
            Some(b'}') => break,
            Some(b'"') => {}
            _ => bail!("a setting's name is expected in quotes"),
        }
        let start = scanner.at;
        let quoted = scanner.string()?;
        // `serde_json` turns the quoted text into the real name, escapes
        // (`\"`, `\u00e9`) included.
        let key: String = serde_json::from_str(&text[quoted])
            .map_err(|error| anyhow!("a setting's name cannot be read: {error}"))?;
        scanner.skip_blank()?;
        if scanner.peek() != Some(b':') {
            bail!("`:` is expected after \"{key}\"");
        }
        scanner.at += 1;
        scanner.skip_blank()?;
        let value = scanner.value()?;
        scanner.skip_blank()?;
        let comma = (scanner.peek() == Some(b',')).then_some(scanner.at);
        members.push(Member {
            key,
            start,
            value,
            comma,
        });
        match comma {
            Some(_) => scanner.at += 1,
            None => {
                if scanner.peek() != Some(b'}') {
                    bail!("`,` or `}}` is expected after a value");
                }
                break;
            }
        }
    }
    scanner.at += 1;
    scanner.skip_blank()?;
    if scanner.peek().is_some() {
        bail!("something follows the closing `}}`");
    }
    Ok(Some(Document { open, members }))
}

/// The value of a top-level setting, as written in the file (a text keeps
/// its quotes). `None` when the setting is not there.
pub(crate) fn get(text: &str, key: &str) -> Result<Option<String>> {
    let Some(document) = scan(text)? else {
        return Ok(None);
    };
    Ok(document
        .find(key)
        .map(|index| text[document.members[index].value.clone()].to_string()))
}

/// The text with a top-level setting set to a text value.
///
/// A setting that is already there gets its value replaced, in place. A new
/// one is written first in the object: nothing already there moves, and a
/// comment stays next to the line it was written for.
pub(crate) fn set(text: &str, key: &str, value: &str) -> Result<String> {
    let key_text = serde_json::to_string(key)?;
    let value_text = serde_json::to_string(value)?;
    // The file's own line ending, so a Windows file stays a Windows file.
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };

    let Some(document) = scan(text)? else {
        // No object yet. Comments, if any, are kept above the new one.
        let mut result = text.to_string();
        if !result.is_empty() && !result.ends_with('\n') {
            result.push_str(newline);
        }
        result.push_str(&format!(
            "{{{newline}    {key_text}: {value_text}{newline}}}{newline}"
        ));
        return Ok(result);
    };

    if let Some(index) = document.find(key) {
        let mut result = text.to_string();
        result.replace_range(document.members[index].value.clone(), &value_text);
        return Ok(result);
    }

    // The indentation the file already uses, read in front of its first
    // setting; four spaces, the editors' default, when there is none.
    let indent = document
        .members
        .first()
        .and_then(|member| {
            let line_start = text[..member.start].rfind('\n')? + 1;
            let before = &text[line_start..member.start];
            before
                .chars()
                .all(|c| c == ' ' || c == '\t')
                .then_some(before)
        })
        .unwrap_or("    ");

    let has_others = !document.members.is_empty();
    let after_brace = document.open + 1;
    // What shares the line of the `{`: when something does (`{}`, or a
    // file written on one line), it is pushed to a line of its own.
    let same_line = text[after_brace..].lines().next().unwrap_or("");
    let mut added = format!("{newline}{indent}{key_text}: {value_text}");
    if has_others {
        added.push(',');
    }
    if !same_line.trim().is_empty() {
        added.push_str(newline);
        if has_others {
            added.push_str(indent);
        }
    }
    let mut result = text.to_string();
    result.insert_str(after_brace, &added);
    Ok(result)
}

/// The text without a top-level setting. `None` when it is not there.
///
/// The setting goes with its comma, and with its whole line when it was
/// alone on it: removing what [`set`] added gives the file back as it was.
pub(crate) fn remove(text: &str, key: &str) -> Result<Option<String>> {
    let Some(document) = scan(text)? else {
        return Ok(None);
    };
    let Some(index) = document.find(key) else {
        return Ok(None);
    };
    let member = &document.members[index];
    let mut start = member.start;
    let mut end = member.value.end;
    // The comma that goes too: the one after the setting, or, for the last
    // setting of the object, the one before it.
    let mut comma_before = None;
    match member.comma {
        Some(comma) => end = comma + 1,
        None if index > 0 => comma_before = document.members[index - 1].comma,
        None => {}
    }

    // Alone on its line? Then the line goes, indentation and line break
    // included.
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |at| end + at + 1);
    let blank = |part: &str| part.chars().all(|c| c.is_ascii_whitespace());
    if blank(&text[line_start..start]) && blank(&text[end..line_end]) && line_start > 0 {
        start = line_start;
        end = line_end;
    }

    let mut result = text.to_string();
    // The later part first: removing it does not move what is before it.
    result.replace_range(start..end, "");
    if let Some(comma) = comma_before {
        result.replace_range(comma..comma + 1, "");
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "claudeCode.claudeProcessWrapper";
    const VALUE: &str = "/home/me/.orberus/ide/claude-wrapper";

    /// A settings file the way people really keep them.
    const REAL: &str = r#"{
    // Fonts
    "editor.fontSize": 13,
    "files.exclude": { "**/.git": true, "a}b": "// not a comment" }, /* inline */
    "terminal.integrated.env.osx": {
        "FOO": "{[\"quoted\"]}"
    },
    "window.zoomLevel": 1, // a comma after the last one is fine here
}
"#;

    fn as_text(raw: Option<String>) -> Option<String> {
        raw.map(|raw| serde_json::from_str(&raw).unwrap())
    }

    #[test]
    fn reads_a_setting_among_comments_and_nested_values() {
        assert_eq!(get(REAL, "editor.fontSize").unwrap().unwrap(), "13");
        assert_eq!(get(REAL, "window.zoomLevel").unwrap().unwrap(), "1");
        assert_eq!(get(REAL, KEY).unwrap(), None);
        // A name inside a nested object is not a top-level setting.
        assert_eq!(get(REAL, "FOO").unwrap(), None);
    }

    #[test]
    fn a_new_setting_goes_first_and_removing_it_gives_the_file_back() {
        for before in [
            REAL,
            "{\n    \"a\": 1\n}\n",
            "{\n  \"a\": 1,\n  \"b\": [1, 2]\n}",
            "{\n}\n",
            "{\r\n    \"a\": 1\r\n}\r\n",
            "\u{feff}{\n\t\"a\": 1\n}\n",
            "{\n    // nothing yet\n}\n",
        ] {
            let with = set(before, KEY, VALUE).unwrap();
            assert_eq!(as_text(get(&with, KEY).unwrap()).unwrap(), VALUE);
            // Everything that was there is still there, in order.
            assert_eq!(get(&with, "a").unwrap(), get(before, "a").unwrap());
            assert_eq!(remove(&with, KEY).unwrap().unwrap(), before, "{before:?}");
        }
    }

    #[test]
    fn it_follows_the_indentation_and_line_endings_of_the_file() {
        let with = set("{\n  \"a\": 1\n}\n", KEY, VALUE).unwrap();
        assert_eq!(
            with,
            format!("{{\n  \"{KEY}\": \"{VALUE}\",\n  \"a\": 1\n}}\n")
        );
        let windows = set("{\r\n    \"a\": 1\r\n}\r\n", KEY, VALUE).unwrap();
        assert!(windows.starts_with(&format!("{{\r\n    \"{KEY}\": \"{VALUE}\",\r\n")));
        assert!(!windows.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn an_empty_file_or_an_empty_object_gets_a_clean_one() {
        let fresh = format!("{{\n    \"{KEY}\": \"{VALUE}\"\n}}\n");
        assert_eq!(set("", KEY, VALUE).unwrap(), fresh);
        assert_eq!(set("{}", KEY, VALUE).unwrap(), fresh.trim_end());
        // Comments with no object are kept above the new object.
        let commented = set("// my settings\n", KEY, VALUE).unwrap();
        assert_eq!(commented, format!("// my settings\n{fresh}"));
        // Removing from the `{}` case leaves an empty object.
        let emptied = remove(&set("{}", KEY, VALUE).unwrap(), KEY)
            .unwrap()
            .unwrap();
        assert_eq!(get(&emptied, KEY).unwrap(), None);
        assert_eq!(emptied, "{\n}");
    }

    #[test]
    fn a_file_on_one_line_stays_valid() {
        let with = set("{\"a\": 1}", KEY, VALUE).unwrap();
        assert_eq!(get(&with, "a").unwrap().unwrap(), "1");
        assert_eq!(as_text(get(&with, KEY).unwrap()).unwrap(), VALUE);
        // Strict JSON in, strict JSON out.
        let parsed: serde_json::Value = serde_json::from_str(&with).unwrap();
        assert_eq!(parsed[KEY], VALUE);
    }

    #[test]
    fn an_existing_setting_is_replaced_in_place() {
        let before =
            format!("{{\n    \"a\": 1,\n    \"{KEY}\": \"/old\", // mine\n    \"b\": 2\n}}\n");
        let after = set(&before, KEY, VALUE).unwrap();
        assert_eq!(after, before.replace("/old", VALUE));
    }

    #[test]
    fn removing_the_last_setting_takes_the_comma_before_it() {
        let before = format!("{{\n    \"a\": 1,\n    \"{KEY}\": \"{VALUE}\"\n}}\n");
        assert_eq!(
            remove(&before, KEY).unwrap().unwrap(),
            "{\n    \"a\": 1\n}\n"
        );
        // Sharing its line with another setting: only the setting goes.
        let shared = format!("{{ \"a\": 1, \"{KEY}\": \"{VALUE}\" }}");
        let after = remove(&shared, KEY).unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(parsed, serde_json::json!({ "a": 1 }));
    }

    #[test]
    fn removing_what_is_not_there_changes_nothing() {
        assert_eq!(remove(REAL, KEY).unwrap(), None);
        assert_eq!(remove("", KEY).unwrap(), None);
    }

    #[test]
    fn a_value_with_special_characters_is_escaped() {
        let value = "C:\\Users\\me\\a \"b\"";
        let with = set("{}", KEY, value).unwrap();
        assert_eq!(as_text(get(&with, KEY).unwrap()).unwrap(), value);
    }

    #[test]
    fn what_it_does_not_understand_is_an_error() {
        for broken in [
            "[1, 2]",
            "{ \"a\": 1",
            "{ \"a\" 1 }",
            "{ \"a\": }",
            "{ a: 1 }",
            "{ \"a\": 1 } trailing",
            "{ \"a\": \"never closed }",
            "{ /* never closed }",
            "{ \"a\": 1 \"b\": 2 }",
        ] {
            assert!(set(broken, KEY, VALUE).is_err(), "{broken}");
            assert!(get(broken, KEY).is_err(), "{broken}");
            assert!(remove(broken, KEY).is_err(), "{broken}");
        }
    }
}

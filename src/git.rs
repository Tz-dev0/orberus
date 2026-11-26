//! Reads git remotes straight from the repository files. No `git` process is
//! started, so the shim stays fast and works where git is not installed.

use std::fs;
use std::path::{Path, PathBuf};

/// Remote URLs of the repository containing `cwd`, normalized.
pub fn remotes(cwd: &Path) -> Vec<String> {
    let Some(git_dir) = find_git_dir(cwd) else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(git_dir.join("config")) else {
        return Vec::new();
    };
    parse_remote_urls(&text)
        .iter()
        .map(|url| normalize_remote(url))
        .collect()
}

/// Finds the folder holding the repository `config` file. Handles normal
/// clones, and worktrees or submodules where `.git` is a file.
fn find_git_dir(cwd: &Path) -> Option<PathBuf> {
    for dir in cwd.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            let text = fs::read_to_string(&dot_git).ok()?;
            let target = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
            let git_dir = dir.join(target.trim());
            // A worktree points to the main repository through `commondir`.
            if let Ok(common) = fs::read_to_string(git_dir.join("commondir")) {
                return Some(git_dir.join(common.trim()));
            }
            return Some(git_dir);
        }
    }
    None
}

fn parse_remote_urls(config: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut in_remote = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_remote = line.starts_with("[remote ");
            continue;
        }
        if !in_remote {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == "url" {
                urls.push(value.trim().to_string());
            }
        }
    }
    urls
}

/// Turns any remote form into `host/owner/repo`, lowercase:
/// `git@github.com:Acme/App.git` and `https://github.com/acme/app` both give
/// `github.com/acme/app`.
pub fn normalize_remote(url: &str) -> String {
    let mut text = url.trim().to_lowercase();
    if let Some(index) = text.find("://") {
        text = text[index + 3..].to_string();
    } else if let Some(colon) = text.find(':') {
        // scp-like form: user@host:owner/repo
        if !text[..colon].contains('/') {
            text.replace_range(colon..=colon, "/");
        }
    }
    let host_end = text.find('/').unwrap_or(text.len());
    if let Some(at) = text[..host_end].rfind('@') {
        text = text[at + 1..].to_string();
    }
    let text = text.trim_end_matches('/');
    text.strip_suffix(".git").unwrap_or(text).to_string()
}

/// Wildcard match where `*` stands for any run of characters.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<usize> = None;
    let mut mark = 0;
    while ti < t.len() {
        if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(star_index) = star {
            pi = star_index + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_every_remote_form() {
        for url in [
            "git@github.com:Acme/App.git",
            "https://github.com/acme/app.git",
            "https://token@github.com/acme/app/",
            "ssh://git@github.com/acme/app.git",
        ] {
            assert_eq!(normalize_remote(url), "github.com/acme/app", "{url}");
        }
    }

    #[test]
    fn glob_matches_with_stars() {
        assert!(glob_match("github.com/acme/*", "github.com/acme/app"));
        assert!(glob_match("*/acme/*", "gitlab.com/acme/tools/cli"));
        assert!(glob_match("github.com/acme/app", "github.com/acme/app"));
        assert!(!glob_match("github.com/acme/*", "github.com/acme-labs/app"));
        assert!(!glob_match("github.com/acme/app", "github.com/acme/app2"));
    }

    #[test]
    fn reads_only_remote_urls() {
        let config = "[core]\n\turl = nope\n[remote \"origin\"]\n\turl = git@github.com:acme/app.git\n\tfetch = +refs\n[branch \"main\"]\n\tremote = origin\n";
        assert_eq!(
            parse_remote_urls(config),
            vec!["git@github.com:acme/app.git"]
        );
    }
}

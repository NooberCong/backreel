//! Image providers. Each module turns a [`Query`] into [`Candidate`]s, keeping
//! its JSON parsing in a pure function so it can be tested without a network.

mod bing;
mod commons;
mod wallhaven;

use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Nature,
    City,
    Architecture,
    Space,
    Abstract,
    Minimal,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Nature,
        Category::City,
        Category::Architecture,
        Category::Space,
        Category::Abstract,
        Category::Minimal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Nature => "Nature",
            Category::City => "City",
            Category::Architecture => "Architecture",
            Category::Space => "Space",
            Category::Abstract => "Abstract",
            Category::Minimal => "Minimal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Bing,
    Wallhaven,
    Commons,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Bing, Source::Wallhaven, Source::Commons];

    pub fn label(self) -> &'static str {
        match self {
            Source::Bing => "Bing",
            Source::Wallhaven => "Wallhaven",
            Source::Commons => "Wikimedia",
        }
    }

    /// Whether this source can honour `category`. Bing publishes one curated
    /// daily image, so it sits outside the category system altogether.
    pub fn serves(self, category: Category) -> bool {
        match self {
            Source::Bing => false,
            Source::Wallhaven => true,
            Source::Commons => commons::serves(category),
        }
    }

    pub fn fetch(self, query: &Query) -> Result<Vec<Candidate>> {
        match self {
            Source::Bing => bing::fetch(),
            Source::Wallhaven => wallhaven::fetch(query),
            Source::Commons => commons::fetch(query),
        }
    }
}

/// What one monitor needs from a source.
#[derive(Clone, Copy, Debug)]
pub struct Query {
    pub category: Category,
    pub width: u32,
    pub height: u32,
}

/// An image a source offers, not yet downloaded.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub source: Source,
    /// Stable across runs; used to avoid repeats and to name the file.
    pub id: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub credit: String,
    pub page_url: String,
}

/// Percent-encodes a query-string value.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_escapes_reserved_and_multibyte() {
        assert_eq!(encode("a b:>é"), "a%20b%3A%3E%C3%A9");
        assert_eq!(encode("safe-_.~"), "safe-_.~");
    }

    #[test]
    fn bing_serves_no_category() {
        assert!(Category::ALL.iter().all(|c| !Source::Bing.serves(*c)));
    }
}

//! Wallhaven's public search API. `purity=100` restricts results to SFW and
//! `categories=100` to its General section (no anime or people).

use super::{Candidate, Category, Query, Source, encode};
use crate::{Result, http};
use serde::Deserialize;

const SEARCH: &str = "https://wallhaven.cc/api/v1/search";
/// Larger files are skipped rather than downloaded.
const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Deserialize)]
struct Results {
    data: Vec<Wallpaper>,
}

#[derive(Deserialize)]
struct Wallpaper {
    id: String,
    url: String,
    path: String,
    dimension_x: u32,
    dimension_y: u32,
    file_size: u64,
}

pub fn fetch(query: &Query) -> Result<Vec<Candidate>> {
    parse(&http::get(&search_url(query), http::MAX_JSON_BYTES)?, query.category)
}

fn search_term(category: Category) -> &'static str {
    match category {
        Category::Nature => "nature",
        Category::City => "city",
        Category::Architecture => "architecture",
        Category::Space => "space",
        Category::Abstract => "abstract",
        Category::Minimal => "minimalism",
    }
}

fn search_url(query: &Query) -> String {
    let ratio = if query.width >= query.height { "landscape" } else { "portrait" };
    format!(
        "{SEARCH}?q={}&categories=100&purity=100&sorting=random&ratios={ratio}&atleast={}x{}",
        encode(search_term(query.category)),
        query.width,
        query.height,
    )
}

fn parse(json: &[u8], category: Category) -> Result<Vec<Candidate>> {
    let results: Results = serde_json::from_slice(json)?;
    Ok(results
        .data
        .into_iter()
        .filter(|w| w.file_size <= MAX_FILE_BYTES)
        .map(|w| Candidate {
            source: Source::Wallhaven,
            id: w.id,
            url: w.path,
            width: w.dimension_x,
            height: w.dimension_y,
            // Search results carry no title or artist.
            title: category.label().to_owned(),
            credit: Source::Wallhaven.label().to_owned(),
            page_url: w.url,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_search_for_monitor() {
        let url = search_url(&Query { category: Category::Minimal, width: 2560, height: 1440 });
        assert!(url.contains("q=minimalism"));
        assert!(url.contains("purity=100"));
        assert!(url.contains("ratios=landscape"));
        assert!(url.contains("atleast=2560x1440"));
    }

    #[test]
    fn portrait_monitor_asks_for_portrait() {
        let url = search_url(&Query { category: Category::City, width: 1440, height: 2560 });
        assert!(url.contains("ratios=portrait"));
    }

    #[test]
    fn parses_and_skips_oversized_files() {
        let json = r#"{"data":[
          {"id":"a1","url":"https://wallhaven.cc/w/a1","path":"https://w.wallhaven.cc/full/a1.jpg",
           "dimension_x":3840,"dimension_y":2160,"file_size":1000},
          {"id":"b2","url":"https://wallhaven.cc/w/b2","path":"https://w.wallhaven.cc/full/b2.png",
           "dimension_x":3840,"dimension_y":2160,"file_size":99999999}],"meta":{}}"#;
        let found = parse(json.as_bytes(), Category::Space).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "a1");
        assert_eq!(found[0].title, "Space");
        assert_eq!(found[0].page_url, "https://wallhaven.cc/w/a1");
    }
}

//! Wikimedia Commons featured pictures: community-reviewed and freely
//! licensed, but only deep enough for landscapes and cityscapes.

use super::{Candidate, Category, Query, Source, encode};
use crate::{Result, http};
use serde::Deserialize;
use std::collections::HashMap;

const API: &str = "https://commons.wikimedia.org/w/api.php";
/// Commons only renders thumbnails at fixed steps; these two are known to work.
const THUMB_WIDTHS: [u32; 2] = [1920, 3840];

pub fn serves(category: Category) -> bool {
    commons_category(category).is_some()
}

fn commons_category(category: Category) -> Option<&'static str> {
    match category {
        Category::Nature => Some("Featured pictures of landscapes"),
        Category::City => Some("Featured pictures of cityscapes"),
        _ => None,
    }
}

#[derive(Deserialize)]
struct Response {
    query: Option<Pages>,
}

#[derive(Deserialize)]
struct Pages {
    pages: HashMap<String, Page>,
}

#[derive(Deserialize)]
struct Page {
    pageid: u64,
    title: String,
    #[serde(default)]
    imageinfo: Vec<ImageInfo>,
}

#[derive(Deserialize)]
struct ImageInfo {
    thumburl: String,
    thumbwidth: u32,
    thumbheight: u32,
    descriptionurl: String,
    #[serde(default)]
    extmetadata: Metadata,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    artist: Option<Field>,
    license_short_name: Option<Field>,
}

#[derive(Deserialize)]
struct Field {
    value: String,
}

pub fn fetch(query: &Query) -> Result<Vec<Candidate>> {
    let Some(category) = commons_category(query.category) else {
        return Ok(Vec::new());
    };
    parse(&http::get(&search_url(category, query.width), http::MAX_JSON_BYTES)?)
}

fn search_url(category: &str, width: u32) -> String {
    let search = format!("incategory:\"{category}\" filew:>{} filemime:image/jpeg", width.saturating_sub(1));
    let thumb = THUMB_WIDTHS.into_iter().find(|w| *w >= width).unwrap_or(THUMB_WIDTHS[1]);
    format!(
        "{API}?action=query&format=json&generator=search&gsrnamespace=6&gsrlimit=20&gsrsort=random\
         &gsrsearch={}&prop=imageinfo&iiprop=url%7Csize%7Cextmetadata\
         &iiextmetadatafilter=Artist%7CLicenseShortName&iiurlwidth={thumb}",
        encode(&search),
    )
}

fn parse(json: &[u8]) -> Result<Vec<Candidate>> {
    let response: Response = serde_json::from_slice(json)?;
    let pages = response.query.map(|q| q.pages).unwrap_or_default();
    Ok(pages.into_values().filter_map(candidate).collect())
}

fn candidate(page: Page) -> Option<Candidate> {
    let info = page.imageinfo.into_iter().next()?;
    let artist = info.extmetadata.artist.map(|f| strip_tags(&f.value));
    let license = info.extmetadata.license_short_name.map(|f| f.value);
    let credit = [artist, license].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    Some(Candidate {
        source: Source::Commons,
        id: page.pageid.to_string(),
        url: info.thumburl,
        width: info.thumbwidth,
        height: info.thumbheight,
        title: file_title(&page.title),
        credit,
        page_url: info.descriptionurl,
    })
}

/// "File:Seven Sisters.jpg" becomes "Seven Sisters".
fn file_title(title: &str) -> String {
    let name = title.strip_prefix("File:").unwrap_or(title);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem).to_owned()
}

/// The Artist field is HTML, often a link; keep only its text.
fn strip_tags(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_nature_and_city_are_served() {
        let served: Vec<_> = Category::ALL.into_iter().filter(|c| serves(*c)).collect();
        assert_eq!(served, [Category::Nature, Category::City]);
    }

    #[test]
    fn search_filters_by_width_and_picks_a_valid_thumbnail() {
        let url = search_url("Featured pictures of landscapes", 2560);
        assert!(url.contains("filew%3A%3E2559"));
        assert!(url.contains("iiurlwidth=3840"));
        assert!(search_url("x", 1920).contains("iiurlwidth=1920"));
        assert!(search_url("x", 5120).contains("iiurlwidth=3840"));
    }

    #[test]
    fn parses_page_with_html_artist() {
        let json = r#"{"query":{"pages":{"1":{"pageid":1,"title":"File:Seven Sisters.v2.jpg","imageinfo":[{
          "thumburl":"https://thumb.wikimedia.org/a.jpg","thumbwidth":3840,"thumbheight":2160,
          "descriptionurl":"https://commons.wikimedia.org/wiki/File:a.jpg",
          "extmetadata":{"Artist":{"value":"<a href=\"x\">Jane  Doe</a>\n"},
                         "LicenseShortName":{"value":"CC BY-SA 4.0"}}}]}}}}"#;
        let found = parse(json.as_bytes()).unwrap();
        assert_eq!(found[0].title, "Seven Sisters.v2");
        assert_eq!(found[0].credit, "Jane Doe · CC BY-SA 4.0");
        assert_eq!(found[0].width, 3840);
    }

    #[test]
    fn empty_search_is_not_an_error() {
        assert!(parse(br#"{"batchcomplete":""}"#).unwrap().is_empty());
    }
}

//! Bing's homepage image feed. Unofficial, keyless, and limited by Microsoft's
//! terms to noncommercial personal use.

use super::{Candidate, Source};
use crate::{Result, http};
use serde::Deserialize;

const FEED: &str = "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt=en-US";
const HOST: &str = "https://www.bing.com";

#[derive(Deserialize)]
struct Feed {
    images: Vec<Image>,
}

#[derive(Deserialize)]
struct Image {
    urlbase: String,
    title: String,
    copyright: String,
    copyrightlink: String,
}

pub fn fetch() -> Result<Vec<Candidate>> {
    parse(&http::get(FEED, http::MAX_JSON_BYTES)?)
}

fn parse(json: &[u8]) -> Result<Vec<Candidate>> {
    let feed: Feed = serde_json::from_slice(json)?;
    Ok(feed.images.into_iter().map(candidate).collect())
}

fn candidate(image: Image) -> Candidate {
    // `urlbase` looks like "/th?id=OHR.Name_EN-US123"; the name is the stable part.
    let id = image.urlbase.rsplit('=').next().unwrap_or(&image.urlbase).to_owned();
    Candidate {
        source: Source::Bing,
        id,
        url: format!("{HOST}{}_UHD.jpg", image.urlbase),
        // The feed reports no dimensions, but UHD images are always this size.
        width: 3840,
        height: 2160,
        title: image.title,
        credit: credit(&image.copyright),
        page_url: image.copyrightlink,
    }
}

/// Extracts "Jackal Pan/Getty Images" from "A lake, China (© Jackal Pan/Getty Images)".
fn credit(copyright: &str) -> String {
    copyright.rsplit_once("(©").map_or(copyright, |(_, rest)| rest.trim_end_matches(')')).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"images":[{
        "urlbase":"/th?id=OHR.AutumnChina_EN-US0467722422",
        "copyright":"People rowing, Anhui, China (© Jackal Pan/Getty Images)",
        "copyrightlink":"https://www.bing.com/search?q=bald+cypress",
        "title":"Paddling a painted forest","wp":true}]}"#;

    #[test]
    fn parses_feed() {
        let found = parse(SAMPLE.as_bytes()).unwrap();
        let image = &found[0];
        assert_eq!(image.id, "OHR.AutumnChina_EN-US0467722422");
        assert_eq!(image.url, "https://www.bing.com/th?id=OHR.AutumnChina_EN-US0467722422_UHD.jpg");
        assert_eq!(image.title, "Paddling a painted forest");
        assert_eq!(image.credit, "Jackal Pan/Getty Images");
    }

    #[test]
    fn credit_falls_back_to_full_text() {
        assert_eq!(credit("Unattributed photo"), "Unattributed photo");
    }

    #[test]
    fn rejects_malformed_feed() {
        assert!(parse(b"{}").is_err());
    }
}

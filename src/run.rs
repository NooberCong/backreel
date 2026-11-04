//! One wallpaper change: choose, download, render, apply.

use crate::desktop::{Desktop, Monitor};
use crate::imaging::{self, Imaging};
use crate::sources::{Candidate, Category, Query, Source};
use crate::store::{self, Applied, Config, State};
use crate::{Result, http};
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::BuildHasher;
use std::path::PathBuf;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// Crop beyond this and too much of the picture is lost.
const MAX_CROP_LOSS: f64 = 0.30;
/// Sources may fall a little short of the monitor; upscaling this far stays sharp enough.
const MIN_COVERAGE: f64 = 0.75;
/// Candidates to attempt per source before moving on.
const MAX_ATTEMPTS: usize = 3;
const WALLPAPERS_KEPT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Task Scheduler: at most one change a day, and never on a metered network.
    Scheduled,
    /// The user asked: always change.
    Manual,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Changed,
    AlreadyDone,
    Metered,
    Busy,
}

/// Runs a change and records the result. State is saved even on failure, so
/// the settings window can say what went wrong.
pub fn run(trigger: Trigger) -> Result<Outcome> {
    let Some(_lock) = RunLock::acquire()? else { return Ok(Outcome::Busy) };
    let mut state = State::load();
    let today = store::today();

    if trigger == Trigger::Scheduled {
        if state.last_change.as_deref() == Some(today.as_str()) {
            return Ok(Outcome::AlreadyDone);
        }
        if http::is_metered() {
            return Ok(Outcome::Metered);
        }
    }

    let result = change_wallpapers(&Config::load(), &mut state);
    match &result {
        Ok(()) => {
            state.last_change = Some(today);
            state.last_error = None;
        }
        Err(error) => state.last_error = Some(error.to_string()),
    }
    state.save()?;
    result.map(|()| Outcome::Changed)
}

fn change_wallpapers(config: &Config, state: &mut State) -> Result<()> {
    let desktop = Desktop::new()?;
    let imaging = Imaging::new()?;
    let monitors = desktop.monitors()?;
    if monitors.is_empty() {
        return Err("no monitors found".into());
    }
    fs::create_dir_all(store::wallpaper_dir())?;

    for monitor in &monitors {
        let (candidate, path) = find_and_render(config, state, monitor, &imaging)?;
        desktop.set(monitor, &path)?;
        store::log(&format!("{:?} {} on {}x{}", candidate.source, candidate.id, monitor.width, monitor.height));
        state.remember(seen_key(&candidate));
        state.current.retain(|applied| applied.monitor != monitor.id);
        state.current.push(Applied {
            monitor: monitor.id.clone(),
            path,
            title: candidate.title,
            credit: candidate.credit,
            page_url: candidate.page_url,
        });
    }

    let protected: Vec<_> = state.current.iter().map(|applied| applied.path.as_path()).collect();
    store::prune_wallpapers(WALLPAPERS_KEPT, &protected)
}

/// Walks the sources until one yields an image that decodes and fits.
fn find_and_render(
    config: &Config,
    state: &State,
    monitor: &Monitor,
    imaging: &Imaging,
) -> Result<(Candidate, PathBuf)> {
    let mut last_error = None;
    let mut tried_any = false;

    for source in source_order(&config.sources, random) {
        let Some(category) = choose_category(source, &config.categories, random) else { continue };
        tried_any = true;
        let query = Query { category, width: monitor.width, height: monitor.height };
        match try_source(source, &query, state, imaging) {
            Ok(Some(found)) => return Ok(found),
            Ok(None) => store::log(&format!("{source:?}: nothing suitable")),
            Err(error) => {
                store::log(&format!("{source:?}: {error}"));
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        let reason = if tried_any { "No new image found" } else { "No source offers these categories" };
        reason.into()
    }))
}

fn try_source(source: Source, query: &Query, state: &State, imaging: &Imaging) -> Result<Option<(Candidate, PathBuf)>> {
    let mut last_error = None;
    let suitable = source.fetch(query)?.into_iter().filter(|candidate| is_suitable(candidate, query, &state.seen));

    for candidate in suitable.take(MAX_ATTEMPTS) {
        let path = store::wallpaper_dir().join(file_name(&candidate, query));
        let rendered = http::get(&candidate.url, http::MAX_IMAGE_BYTES)
            .and_then(|bytes| imaging.render_jpeg(&bytes, (query.width, query.height), &path));
        match rendered {
            Ok(()) => return Ok(Some((candidate, path))),
            Err(error) => {
                let _ = fs::remove_file(&path);
                store::log(&format!("{}: {error}", candidate.url));
                last_error = Some(error);
            }
        }
    }
    last_error.map_or(Ok(None), Err)
}

fn is_suitable(candidate: &Candidate, query: &Query, seen: &[String]) -> bool {
    let large_enough = f64::from(candidate.width) >= f64::from(query.width) * MIN_COVERAGE
        && f64::from(candidate.height) >= f64::from(query.height) * MIN_COVERAGE;
    let fits = imaging::crop_loss((candidate.width, candidate.height), (query.width, query.height)) <= MAX_CROP_LOSS;
    large_enough && fits && !seen.contains(&seen_key(candidate))
}

fn seen_key(candidate: &Candidate) -> String {
    format!("{:?}:{}", candidate.source, candidate.id)
}

/// Unique per image and size: Windows may ignore a reused wallpaper file name.
fn file_name(candidate: &Candidate, query: &Query) -> String {
    let id: String = candidate.id.chars().filter(char::is_ascii_alphanumeric).collect();
    format!("{:?}-{id}-{}x{}.jpg", candidate.source, query.width, query.height).to_lowercase()
}

/// Bing and Wallhaven share the days at random; Commons is the fallback.
fn source_order(enabled: &[Source], mut random: impl FnMut() -> u64) -> Vec<Source> {
    let mut primary: Vec<Source> = enabled.iter().copied().filter(|s| *s != Source::Commons).collect();
    for index in (1..primary.len()).rev() {
        primary.swap(index, (random() % (index as u64 + 1)) as usize);
    }
    if enabled.contains(&Source::Commons) {
        primary.push(Source::Commons);
    }
    primary
}

/// Picks one of the user's categories that `source` can serve. Bing has no
/// categories, so whatever is returned for it goes unused.
fn choose_category(source: Source, categories: &[Category], mut random: impl FnMut() -> u64) -> Option<Category> {
    if source == Source::Bing {
        return categories.first().copied();
    }
    let served: Vec<_> = categories.iter().copied().filter(|c| source.serves(*c)).collect();
    if served.is_empty() { None } else { Some(served[(random() % served.len() as u64) as usize]) }
}

fn random() -> u64 {
    RandomState::new().hash_one(0u8)
}

/// Keeps a scheduled run and a manual run from overlapping.
struct RunLock(HANDLE);

impl RunLock {
    fn acquire() -> Result<Option<Self>> {
        // SAFETY: the name is a static string; the handle is owned by `Self`.
        unsafe {
            let handle = CreateMutexW(None, true, w!("Local\\backreel.run"))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Ok(None);
            }
            Ok(Some(Self(handle)))
        }
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and closed exactly once. Closing it also
        // abandons the mutex, which is all a named-existence lock needs.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(width: u32, height: u32) -> Candidate {
        Candidate {
            source: Source::Wallhaven,
            id: "abc".to_owned(),
            url: String::new(),
            width,
            height,
            title: String::new(),
            credit: String::new(),
            page_url: String::new(),
        }
    }

    const QHD: Query = Query { category: Category::Nature, width: 2560, height: 1440 };

    #[test]
    fn accepts_a_matching_image() {
        assert!(is_suitable(&candidate(3840, 2160), &QHD, &[]));
    }

    #[test]
    fn rejects_small_images() {
        assert!(!is_suitable(&candidate(1280, 720), &QHD, &[]));
    }

    #[test]
    fn rejects_images_that_lose_too_much_to_cropping() {
        assert!(!is_suitable(&candidate(7777, 2000), &QHD, &[]));
        let portrait = Query { width: 1440, height: 2560, ..QHD };
        assert!(!is_suitable(&candidate(3840, 2160), &portrait, &[]));
    }

    #[test]
    fn rejects_images_already_seen() {
        let image = candidate(3840, 2160);
        assert!(!is_suitable(&image, &QHD, &[seen_key(&image)]));
    }

    #[test]
    fn commons_always_comes_last() {
        for seed in 0..8 {
            let order = source_order(&Source::ALL, || seed);
            assert_eq!(order.len(), 3);
            assert_eq!(order[2], Source::Commons);
        }
    }

    #[test]
    fn both_primary_sources_get_a_turn_first() {
        assert_eq!(source_order(&Source::ALL, || 0)[0], Source::Wallhaven);
        assert_eq!(source_order(&Source::ALL, || 1)[0], Source::Bing);
    }

    #[test]
    fn disabled_sources_are_left_out() {
        assert_eq!(source_order(&[Source::Commons], || 0), [Source::Commons]);
        assert_eq!(source_order(&[Source::Bing], || 0), [Source::Bing]);
    }

    #[test]
    fn commons_is_skipped_when_no_category_fits() {
        assert_eq!(choose_category(Source::Commons, &[Category::Space], || 0), None);
        assert_eq!(choose_category(Source::Commons, &[Category::Space, Category::City], || 0), Some(Category::City));
    }

    #[test]
    fn file_names_are_safe_and_differ_by_size() {
        let mut image = candidate(1, 1);
        image.id = "OHR.A/b:c".to_owned();
        assert_eq!(file_name(&image, &QHD), "wallhaven-ohrabc-2560x1440.jpg");
    }
}

//! Everything backreel keeps on disk, all under one folder.

use crate::Result;
use crate::sources::{Category, Source};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use windows::Win32::System::SystemInformation::GetLocalTime;

const MAX_LOG_BYTES: u64 = 64 * 1024;
/// How many image ids to remember so that repeats stay rare.
const MAX_SEEN: usize = 500;

/// `%LOCALAPPDATA%\backreel`, or `BACKREEL_HOME` when set (used by tests).
pub fn home() -> PathBuf {
    let base = std::env::var_os("BACKREEL_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("backreel")));
    base.unwrap_or_else(|| PathBuf::from("backreel"))
}

pub fn wallpaper_dir() -> PathBuf {
    home().join("wallpapers")
}

/// The user's choices. Saved the moment they change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub categories: Vec<Category>,
    pub sources: Vec<Source>,
}

impl Default for Config {
    fn default() -> Self {
        Self { categories: vec![Category::Nature, Category::City, Category::Space], sources: Source::ALL.to_vec() }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config: Self = load_json("config.json");
        let defaults = Self::default();
        if config.categories.is_empty() {
            config.categories = defaults.categories;
        }
        if config.sources.is_empty() {
            config.sources = defaults.sources;
        }
        config
    }

    pub fn save(&self) -> Result<()> {
        save_json("config.json", self)
    }

    /// Flips `category`, refusing to remove the last one.
    pub fn toggle_category(&mut self, category: Category) {
        toggle(&mut self.categories, category);
    }

    /// Flips `source`, refusing to remove the last one.
    pub fn toggle_source(&mut self, source: Source) {
        toggle(&mut self.sources, source);
    }
}

fn toggle<T: PartialEq>(items: &mut Vec<T>, item: T) {
    match items.iter().position(|existing| *existing == item) {
        Some(_) if items.len() == 1 => {}
        Some(index) => {
            items.remove(index);
        }
        None => items.push(item),
    }
}

/// The wallpaper backreel put on one monitor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub monitor: String,
    pub path: PathBuf,
    pub title: String,
    pub credit: String,
    pub page_url: String,
}

/// What backreel has done, as opposed to what the user asked for.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Local date of the last complete change, as YYYY-MM-DD.
    pub last_change: Option<String>,
    pub last_error: Option<String>,
    pub seen: Vec<String>,
    pub current: Vec<Applied>,
}

impl State {
    pub fn load() -> Self {
        load_json("state.json")
    }

    pub fn save(&self) -> Result<()> {
        save_json("state.json", self)
    }

    pub fn remember(&mut self, id: String) {
        self.seen.push(id);
        let excess = self.seen.len().saturating_sub(MAX_SEEN);
        self.seen.drain(..excess);
    }
}

/// A missing or corrupt file falls back to defaults; backreel must always start.
fn load_json<T: DeserializeOwned + Default>(name: &str) -> T {
    let path = home().join(name);
    let Ok(bytes) = fs::read(&path) else { return T::default() };
    serde_json::from_slice(&bytes).unwrap_or_else(|error| {
        log(&format!("ignoring {}: {error}", path.display()));
        T::default()
    })
}

/// Writes through a temporary file, so a killed run cannot leave half a file.
fn save_json<T: Serialize>(name: &str, value: &T) -> Result<()> {
    let dir = home();
    fs::create_dir_all(&dir)?;
    let temporary = dir.join(format!("{name}.tmp"));
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temporary, dir.join(name))?;
    Ok(())
}

/// Deletes all but the `keep` newest wallpapers, sparing any in `protected`.
pub fn prune_wallpapers(keep: usize, protected: &[&Path]) -> Result<()> {
    let mut files: Vec<_> = fs::read_dir(wallpaper_dir())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    files.sort_by_key(|(modified, _)| Reverse(*modified));
    for (_, path) in files.into_iter().skip(keep) {
        if !protected.contains(&path.as_path()) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Today's local date as YYYY-MM-DD.
pub fn today() -> String {
    // SAFETY: GetLocalTime has no preconditions.
    let now = unsafe { GetLocalTime() };
    format!("{:04}-{:02}-{:02}", now.wYear, now.wMonth, now.wDay)
}

/// Appends to the log. There is no console, and a failure to log must not stop a run.
pub fn log(message: &str) {
    let dir = home();
    let path = dir.join("backreel.log");
    let _ = fs::create_dir_all(&dir);
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        let _ = fs::remove_file(&path);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        // SAFETY: GetLocalTime has no preconditions.
        let now = unsafe { GetLocalTime() };
        let _ = writeln!(
            file,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {message}",
            now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_adds_and_removes() {
        let mut config = Config::default();
        config.toggle_category(Category::Abstract);
        assert!(config.categories.contains(&Category::Abstract));
        config.toggle_category(Category::Abstract);
        assert!(!config.categories.contains(&Category::Abstract));
    }

    #[test]
    fn the_last_choice_cannot_be_removed() {
        let mut config = Config { categories: vec![Category::Space], sources: vec![Source::Bing] };
        config.toggle_category(Category::Space);
        config.toggle_source(Source::Bing);
        assert_eq!(config.categories, [Category::Space]);
        assert_eq!(config.sources, [Source::Bing]);
    }

    #[test]
    fn seen_list_is_bounded_and_keeps_the_newest() {
        let mut state = State::default();
        for id in 0..MAX_SEEN + 10 {
            state.remember(id.to_string());
        }
        assert_eq!(state.seen.len(), MAX_SEEN);
        assert_eq!(state.seen.first().unwrap(), "10");
    }

    #[test]
    fn unknown_config_fields_and_missing_fields_are_tolerated() {
        let config: Config = serde_json::from_str(r#"{"sources":["bing"],"future":1}"#).unwrap();
        assert_eq!(config.sources, [Source::Bing]);
        assert_eq!(config.categories, Config::default().categories);
    }

    #[test]
    fn today_is_iso_formatted() {
        let date = today();
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
    }
}

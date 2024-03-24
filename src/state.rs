use std::{collections::HashSet, fs::File, path::PathBuf};

use color_eyre::eyre::{Context, OptionExt, Result};
use mpd::Song;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    #[serde(skip)]
    persist: bool,
    already_played: HashSet<String>,
}

impl AppState {
    fn path() -> Result<PathBuf> {
        let mut path = dirs::data_dir().ok_or_eyre("no suitable data directory found")?;

        path.push("rshuffle");
        path.push("state.json");

        Ok(path)
    }

    pub fn load(persist: bool) -> Result<Self> {
        let path = Self::path()?;

        if !persist || !path.exists() {
            let state = Self {
                persist,
                ..Default::default()
            };

            return Ok(state);
        }

        let reader = File::open(path).wrap_err("failed to open state file for reading")?;

        let mut state: Self =
            serde_json::from_reader(reader).wrap_err("failed to deserialize app state")?;

        state.persist = persist;

        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        if !self.persist {
            return Ok(());
        }

        let path = Self::path()?;

        if !path.exists() {
            std::fs::create_dir_all(path.parent().unwrap())
                .wrap_err("failed to create state directory")?;
        }

        let writer = File::create(path).wrap_err("failed to open state file for writing")?;

        serde_json::to_writer(writer, self).wrap_err("failed to serialize app state")?;

        Ok(())
    }

    pub fn has_been_played(&self, song: &Song) -> bool {
        self.already_played.contains(&song.file)
    }

    pub fn mark_as_played(&mut self, song: &Song) {
        self.already_played.insert(song.file.clone());
    }

    pub fn clear(&mut self) {
        self.already_played.clear();
    }
}

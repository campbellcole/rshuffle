use std::{collections::HashSet, path::PathBuf};

use color_eyre::eyre::{Context, OptionExt, Result};
use mpd_client::responses::Song;
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

    pub async fn load(persist: bool) -> Result<Self> {
        let path = Self::path()?;

        if !persist || !path.exists() {
            let state = Self {
                persist,
                ..Default::default()
            };

            return Ok(state);
        }

        let state = tokio::fs::read_to_string(path)
            .await
            .wrap_err("failed to open state file for reading")?;

        let mut state: Self =
            serde_json::from_str(&state).wrap_err("failed to deserialize app state")?;

        state.persist = persist;

        Ok(state)
    }

    pub async fn save(&self) -> Result<()> {
        if !self.persist {
            return Ok(());
        }

        let path = Self::path()?;

        if !path.exists() {
            tokio::fs::create_dir_all(path.parent().unwrap())
                .await
                .wrap_err("failed to create state directory")?;
        }

        let state = serde_json::to_string(self).wrap_err("failed to serialize app state")?;

        tokio::fs::write(path, state)
            .await
            .wrap_err("failed to write state")?;

        Ok(())
    }

    pub fn has_been_played(&self, song: &Song) -> bool {
        self.already_played.contains(&song.url)
    }

    pub fn mark_as_played(&mut self, song: &Song) {
        self.already_played.insert(song.url.clone());
    }

    pub fn clear(&mut self) {
        self.already_played.clear();
    }
}

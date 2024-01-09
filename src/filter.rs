use std::str::FromStr;

use mpd::Song;

#[derive(Debug, Clone, Copy)]
pub enum FilterField {
    Title,
    Artist,
    Album,
    Any,
}

#[derive(Debug)]
pub enum FilterError {
    InvalidField,
    InvalidValue,
}

impl FromStr for FilterField {
    type Err = FilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "title" => Ok(FilterField::Title),
            "artist" => Ok(FilterField::Artist),
            "album" => Ok(FilterField::Album),
            "any" => Ok(FilterField::Any),
            _ => Err(FilterError::InvalidField),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Filter {
    field: FilterField,
    value: String,
}

impl FromStr for Filter {
    type Err = FilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.contains(':') {
            return Ok(Filter {
                field: FilterField::Any,
                value: s.to_string(),
            });
        }

        let mut parts = s.splitn(2, ':');
        let field = parts.next().ok_or(FilterError::InvalidField)?;
        let value = parts.next().ok_or(FilterError::InvalidValue)?;

        Ok(Filter {
            field: FilterField::from_str(field)?,
            value: value.to_string(),
        })
    }
}

impl Filter {
    pub fn matches(&self, song: &Song) -> bool {
        let to_compare = match self.field {
            FilterField::Title => song.title.as_ref(),
            FilterField::Artist => song.artist.as_ref(),
            FilterField::Album => song
                .tags
                .iter()
                .find(|t| t.0.to_lowercase() == "album")
                .map(|t| &t.1),
            FilterField::Any => {
                return self.as_field(FilterField::Title).matches(song)
                    || self.as_field(FilterField::Artist).matches(song)
                    || self.as_field(FilterField::Album).matches(song)
            }
        };

        to_compare.is_some_and(|s| s.to_lowercase().contains(&self.value))
    }

    pub fn as_field(&self, field: FilterField) -> Self {
        Self {
            field,
            value: self.value.clone(),
        }
    }
}

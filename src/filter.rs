use std::str::FromStr;

use mpd_client::responses::Song;

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
    invert: bool,
}

impl FromStr for Filter {
    type Err = FilterError;

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        if !s.contains(':') {
            let mut invert = false;

            if s.starts_with('!') {
                s = &s[1..];
                invert = true;
            }

            return Ok(Filter {
                field: FilterField::Any,
                value: s.to_string(),
                invert,
            });
        }

        let mut parts = s.splitn(2, ':');
        let mut field = parts.next().ok_or(FilterError::InvalidField)?;
        let value = parts.next().ok_or(FilterError::InvalidValue)?;

        let mut inverted = false;
        if field.starts_with('!') {
            inverted = true;
            field = &field[1..];
        }

        Ok(Filter {
            field: FilterField::from_str(field)?,
            value: value.to_string(),
            invert: inverted,
        })
    }
}

impl Filter {
    pub fn is_inverted(&self) -> bool {
        self.invert
    }

    pub fn matches(&self, song: &Song) -> bool {
        let to_compare = match self.field {
            FilterField::Title => song.title(),
            FilterField::Artist => {
                let artists = song.artists();

                if artists.is_empty() {
                    None
                } else {
                    Some(artists[0].as_str())
                }
            }
            FilterField::Album => song.album(),
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
            invert: self.invert,
        }
    }
}

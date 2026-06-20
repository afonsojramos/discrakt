use chrono::DateTime;

use crate::source::{MediaIds, MediaKind, Source, Watching};
use crate::trakt::{Trakt, TraktWatchingResponse};
use crate::utils::MediaType;

/// A [`Source`] backed by Trakt.tv.
///
/// Wraps the Trakt API client, mapping its `currently watching` response into a
/// source-agnostic [`Watching`] and enriching it with TMDB artwork, localized
/// titles, and (for movies) a Trakt rating.
pub struct TraktSource {
    trakt: Trakt,
    tmdb_token: String,
}

impl TraktSource {
    pub fn new(trakt: Trakt, tmdb_token: String) -> Self {
        TraktSource { trakt, tmdb_token }
    }

    /// Maps a raw Trakt response into an enriched [`Watching`].
    fn enrich(&mut self, response: TraktWatchingResponse) -> Option<Watching> {
        let started_at = DateTime::parse_from_rfc3339(&response.started_at).ok()?;
        let expires_at = DateTime::parse_from_rfc3339(&response.expires_at).ok()?;
        let token = &self.tmdb_token;

        match response.r#type.as_str() {
            "movie" => {
                let movie = response.movie.as_ref()?;
                let slug = movie.ids.slug.clone();

                let rating = slug
                    .as_ref()
                    .map(|slug| self.trakt.get_movie_rating(slug.clone()));

                let (poster_url, title) = match movie.ids.tmdb {
                    Some(tmdb_id) => {
                        let poster = self.trakt.tmdb_mut().get_poster(
                            MediaType::Movie,
                            tmdb_id.to_string(),
                            token,
                            0,
                        );
                        let localized = self.trakt.tmdb_mut().get_title(
                            MediaType::Movie,
                            tmdb_id.to_string(),
                            token,
                            None,
                            None,
                        );
                        let title = if localized.is_empty() {
                            movie.title.clone()
                        } else {
                            localized
                        };
                        (poster, title)
                    }
                    None => (None, movie.title.clone()),
                };

                Some(Watching {
                    kind: MediaKind::Movie,
                    title,
                    year: Some(movie.year),
                    season: None,
                    episode_number: None,
                    episode_title: None,
                    ids: MediaIds {
                        imdb: movie.ids.imdb.clone(),
                        tmdb: movie.ids.tmdb,
                        slug: slug.clone(),
                    },
                    rating,
                    poster_url,
                    imdb_url: movie
                        .ids
                        .imdb
                        .as_ref()
                        .map(|imdb| format!("https://www.imdb.com/title/{}", imdb)),
                    started_at,
                    expires_at,
                    runtime_minutes: movie.runtime.filter(|&r| r > 0),
                })
            }
            "episode" => {
                let show = response.show.as_ref()?;
                let episode = response.episode.as_ref()?;
                let slug = show.ids.slug.clone();

                let mut title = show.title.clone();
                let mut episode_title = episode.title.clone();
                let mut poster_url = None;

                if let Some(tmdb_id) = show.ids.tmdb {
                    poster_url = self.trakt.tmdb_mut().get_poster(
                        MediaType::Show,
                        tmdb_id.to_string(),
                        token,
                        episode.season,
                    );
                    let localized_show = self.trakt.tmdb_mut().get_title(
                        MediaType::Show,
                        tmdb_id.to_string(),
                        token,
                        None,
                        None,
                    );
                    if !localized_show.is_empty() {
                        title = localized_show;
                    }
                    let localized_episode = self.trakt.tmdb_mut().get_title(
                        MediaType::Show,
                        tmdb_id.to_string(),
                        token,
                        Some(episode.season),
                        Some(episode.number),
                    );
                    if !localized_episode.is_empty() {
                        episode_title = localized_episode;
                    }
                }

                let runtime_minutes = episode.runtime.or(show.runtime).filter(|&r| r > 0);

                Some(Watching {
                    kind: MediaKind::Episode,
                    title,
                    year: Some(show.year),
                    season: Some(episode.season),
                    episode_number: Some(episode.number),
                    episode_title: Some(episode_title),
                    ids: MediaIds {
                        imdb: show.ids.imdb.clone(),
                        tmdb: show.ids.tmdb,
                        slug: slug.clone(),
                    },
                    rating: None,
                    poster_url,
                    imdb_url: show
                        .ids
                        .imdb
                        .as_ref()
                        .map(|imdb| format!("https://www.imdb.com/title/{}", imdb)),
                    started_at,
                    expires_at,
                    runtime_minutes,
                })
            }
            other => {
                tracing::warn!("Unknown Trakt media type: {}", other);
                None
            }
        }
    }
}

impl Source for TraktSource {
    fn get_watching(&mut self) -> Option<Watching> {
        let response = self.trakt.get_watching()?;
        self.enrich(response)
    }

    fn set_language(&mut self, language: String) {
        self.trakt.set_language(language);
    }
}

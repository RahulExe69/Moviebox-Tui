//! Stable, UI-friendly facade over the existing provider service.
//!
//! This module deliberately contains no HTTP server or platform-specific code.
//! It gives the future Android bridge and local API one small, typed entry point
//! while keeping all provider behaviour in MovieBoxService.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::BrowseMetrics;
use crate::providers::ProviderCapabilities;
use crate::providers::models::{
    CatalogItem, MediaDetails, PlaybackSource, ProviderError, ProviderKind, Release, SubtitleOption,
};
use crate::service::MovieBoxService;

/// Version of the mobile-facing contract. Bump only when the public contract changes.
pub const MOBILE_API_VERSION: u16 = 1;

/// A provider option suitable for rendering in the mobile UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileProvider {
    pub id: ProviderKind,
    pub name: String,
    pub capabilities: ProviderCapabilities,
    pub available: bool,
}

/// Homepage data returned by the current MovieBox homepage endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileHomepage {
    pub items: Vec<CatalogItem>,
    pub metrics: HashMap<String, BrowseMetrics>,
}

/// Release metadata surfaced to the mobile client before a native player is launched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobilePlaybackResponse {
    pub provider: ProviderKind,
    pub media_id: String,
    /// Movies use 0/0; series use the selected season and episode numbers.
    pub season: usize,
    pub episode: usize,
    pub releases: Vec<Release>,
    pub subtitles: Vec<SubtitleOption>,
    /// Parallel to releases: source provider, media ID, and provider-local release index.
    pub release_sources: Vec<MobileReleaseSource>,
    /// Unique provider/media IDs which contributed playable releases.
    pub provider_sources: Vec<crate::providers::models::ProviderMediaId>,
}

/// Maps one merged playback row back to the provider-specific release index.
/// Resolution can then query just that provider instead of repeating every search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobileReleaseSource {
    pub provider: ProviderKind,
    pub media_id: String,
    pub release_index: usize,
}

/// A merged catalogue entry. One visible title may be backed by several providers
/// and audio cuts; sources retains the provider-specific IDs needed for playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MobileCatalogItem {
    pub id: crate::providers::models::ProviderMediaId,
    pub title: String,
    pub media_type: crate::providers::models::MediaType,
    pub year: Option<String>,
    pub poster_url: Option<String>,
    pub season_count: Option<usize>,
    pub sources: Vec<crate::providers::models::ProviderMediaId>,
    pub languages: Vec<String>,
    pub provider_count: usize,
}

/// Errors at the mobile facade boundary. These are intentionally transport-neutral;
/// a future HTTP adapter can map them to suitable status codes.
#[derive(Debug, Error)]
pub enum MobileApiError {
    #[error("Unknown provider: {0}")]
    UnknownProvider(String),
    #[error("Search query must not be empty")]
    EmptyQuery,
    #[error("Media ID must not be empty")]
    EmptyMediaId,
    #[error("Homepage tab ID must not be empty")]
    EmptyTabId,
    #[error(
        "Choose either a movie (season and episode both zero) or a specific season and episode"
    )]
    InvalidEpisodeSelection,
    #[error("Playback release index is out of range")]
    InvalidReleaseIndex,
    #[error("Subtitle index is out of range")]
    InvalidSubtitleIndex,
    #[error("{error}")]
    PlaybackProvider {
        provider: ProviderKind,
        error: ProviderError,
    },
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("{0}")]
    Service(String),
}

fn strip_trailing_title_tags(value: &str) -> String {
    let mut cleaned = value.trim().to_string();
    loop {
        let Some(open) = cleaned.rfind('[') else {
            break;
        };
        if cleaned.ends_with(']') && open > 0 {
            cleaned.truncate(open);
            cleaned = cleaned.trim().to_string();
        } else {
            break;
        }
    }
    cleaned
}

fn normalize_catalog_title(value: &str) -> String {
    strip_trailing_title_tags(value)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn years_compatible(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) if !left.trim().is_empty() && !right.trim().is_empty() => {
            left.trim() == right.trim()
        }
        _ => true,
    }
}

fn title_languages(value: &str) -> Vec<String> {
    let lower = value.to_ascii_lowercase();
    let known = [
        ("english", "English"),
        ("hindi", "Hindi"),
        ("bengali", "Bengali"),
        ("bangla", "Bengali"),
        ("tamil", "Tamil"),
        ("telugu", "Telugu"),
        ("kannada", "Kannada"),
        ("malayalam", "Malayalam"),
        ("punjabi", "Punjabi"),
        ("urdu", "Urdu"),
        ("japanese", "Japanese"),
        ("korean", "Korean"),
        ("chinese", "Chinese"),
        ("spanish", "Spanish"),
        ("french", "French"),
        ("arabic", "Arabic"),
        ("portuguese", "Portuguese"),
        ("thai", "Thai"),
    ];
    let mut languages = Vec::new();
    for (needle, label) in known {
        if lower.contains(&format!("[{needle}]"))
            && !languages.iter().any(|value: &String| value == label)
        {
            languages.push(label.to_string());
        }
    }
    languages
}

fn provider_count(sources: &[crate::providers::models::ProviderMediaId]) -> usize {
    let mut providers = Vec::new();
    for source in sources {
        if !providers.contains(&source.provider) {
            providers.push(source.provider);
        }
    }
    providers.len()
}

fn mobile_catalog_item(item: CatalogItem) -> MobileCatalogItem {
    let languages = title_languages(&item.title);
    let display_title = strip_trailing_title_tags(&item.title);
    MobileCatalogItem {
        id: item.id.clone(),
        title: if display_title.is_empty() {
            item.title.clone()
        } else {
            display_title
        },
        media_type: item.media_type,
        year: item.year,
        poster_url: item.poster_url,
        season_count: item.season_count,
        sources: vec![item.id],
        languages,
        provider_count: 1,
    }
}

fn playback_search_queries(title: &str) -> Vec<String> {
    let cleaned = strip_trailing_title_tags(title);
    let mut queries = Vec::new();
    if !cleaned.is_empty() {
        queries.push(cleaned);
    }
    if !title.trim().is_empty()
        && !queries
            .iter()
            .any(|query| query.eq_ignore_ascii_case(title.trim()))
    {
        queries.push(title.trim().to_string());
    }
    queries
}

fn media_title_from_id(media_id: &str) -> String {
    media_id.trim().to_string()
}

fn append_unique_releases(
    output: &mut Vec<Release>,
    source_refs: &mut Vec<MobileReleaseSource>,
    seen: &mut std::collections::HashSet<String>,
    provider: ProviderKind,
    media_id: &str,
    candidates: Vec<Release>,
) {
    for (release_index, release) in candidates.into_iter().enumerate() {
        let key = format!(
            "{}|{}|{}|{}|{}|{}",
            release.provider.cache_key(),
            release.filename,
            release.quality.as_deref().unwrap_or_default(),
            release.language.as_deref().unwrap_or_default(),
            release.resource_id.as_deref().unwrap_or_default(),
            release
                .mirrors
                .iter()
                .map(|mirror| mirror.resolver_url.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        if seen.insert(key) {
            output.push(release);
            source_refs.push(MobileReleaseSource {
                provider,
                media_id: media_id.to_string(),
                release_index,
            });
        }
    }
}

/// A small, transport-independent API for a mobile client.
#[derive(Clone)]
pub struct MobileApi {
    service: MovieBoxService,
}

impl Default for MobileApi {
    fn default() -> Self {
        Self::new()
    }
}

impl MobileApi {
    pub fn new() -> Self {
        Self::with_service(MovieBoxService::new())
    }

    pub fn with_service(service: MovieBoxService) -> Self {
        Self { service }
    }

    /// Parse a stable provider ID or a supported provider alias.
    pub fn parse_provider(provider: &str) -> Result<ProviderKind, MobileApiError> {
        ProviderKind::parse(provider)
            .ok_or_else(|| MobileApiError::UnknownProvider(provider.trim().to_string()))
    }

    /// Providers exposed by the existing application, with capabilities sourced
    /// directly from their provider implementations.
    pub fn providers(&self) -> Vec<MobileProvider> {
        ProviderKind::ENABLED
            .iter()
            .copied()
            .map(|id| MobileProvider {
                id,
                name: id.label().to_string(),
                capabilities: self.service.capabilities(id),
                available: match id {
                    ProviderKind::FourKHdHub => self.service.fourk_client.is_some(),
                    _ => true,
                },
            })
            .collect()
    }

    pub async fn search(
        &self,
        provider: &str,
        query: &str,
        page: usize,
    ) -> Result<Vec<CatalogItem>, MobileApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(MobileApiError::EmptyQuery);
        }

        let provider = Self::parse_provider(provider)?;
        self.service
            .search_typed(provider, query, page.clamp(1, 1000))
            .await
            .map_err(MobileApiError::from)
    }

    /// Search a single provider or all enabled providers ("all"), merge duplicate
    /// titles, and retain each provider-specific ID for later playback.
    pub async fn search_catalog(
        &self,
        provider: &str,
        query: &str,
        page: usize,
    ) -> Result<Vec<MobileCatalogItem>, MobileApiError> {
        if provider.trim().eq_ignore_ascii_case("all") {
            return self.search_all(query, page).await;
        }
        let items = self.search(provider, query, page).await?;
        Ok(items.into_iter().map(mobile_catalog_item).collect())
    }

    pub async fn search_all(
        &self,
        query: &str,
        page: usize,
    ) -> Result<Vec<MobileCatalogItem>, MobileApiError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(MobileApiError::EmptyQuery);
        }
        let page = page.clamp(1, 1000);
        let (moviebox, fourk, dramachi, circleftp, dhakaflix) = tokio::join!(
            self.service
                .search_typed(ProviderKind::MovieBox, query, page),
            self.service
                .search_typed(ProviderKind::FourKHdHub, query, page),
            self.service
                .search_typed(ProviderKind::Dramachi, query, page),
            self.service
                .search_typed(ProviderKind::BdixCircleFtp, query, page),
            self.service
                .search_typed(ProviderKind::BdixDhakaFlix, query, page),
        );

        let provider_results = [
            (ProviderKind::MovieBox, moviebox),
            (ProviderKind::FourKHdHub, fourk),
            (ProviderKind::Dramachi, dramachi),
            (ProviderKind::BdixCircleFtp, circleftp),
            (ProviderKind::BdixDhakaFlix, dhakaflix),
        ];
        let mut merged: Vec<MobileCatalogItem> = Vec::new();
        let mut first_error = None;
        for (provider, result) in provider_results {
            match result {
                Ok(items) => {
                    for item in items {
                        let candidate = mobile_catalog_item(item);
                        let normalized = normalize_catalog_title(&candidate.title);
                        let same_index = merged.iter().position(|existing| {
                            normalize_catalog_title(&existing.title) == normalized
                                && existing.media_type == candidate.media_type
                                && years_compatible(
                                    existing.year.as_deref(),
                                    candidate.year.as_deref(),
                                )
                        });
                        if let Some(index) = same_index {
                            let existing = &mut merged[index];
                            for source in candidate.sources {
                                if !existing.sources.contains(&source) {
                                    existing.sources.push(source);
                                }
                            }
                            for language in candidate.languages {
                                if !existing
                                    .languages
                                    .iter()
                                    .any(|value| value.eq_ignore_ascii_case(&language))
                                {
                                    existing.languages.push(language);
                                }
                            }
                            if existing.poster_url.is_none() {
                                existing.poster_url = candidate.poster_url;
                            }
                            if existing.year.is_none() {
                                existing.year = candidate.year;
                            }
                            existing.provider_count = provider_count(&existing.sources);
                        } else {
                            merged.push(candidate);
                        }
                    }
                }
                Err(error) => {
                    log::debug!("catalog search failed for {}: {}", provider, error);
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        if merged.is_empty() {
            if let Some(error) = first_error {
                return Err(MobileApiError::Provider(error));
            }
        }
        for item in &mut merged {
            item.provider_count = provider_count(&item.sources);
        }
        Ok(merged)
    }

    pub async fn details(
        &self,
        provider: &str,
        media_id: &str,
    ) -> Result<MediaDetails, MobileApiError> {
        let media_id = media_id.trim();
        if media_id.is_empty() {
            return Err(MobileApiError::EmptyMediaId);
        }

        let provider = Self::parse_provider(provider)?;
        self.service
            .details_typed(provider, media_id)
            .await
            .map_err(MobileApiError::from)
    }

    /// Return candidate releases, transport headers, and available external subtitles.
    pub async fn playback_sources(
        &self,
        provider: &str,
        media_id: &str,
        season: usize,
        episode: usize,
    ) -> Result<MobilePlaybackResponse, MobileApiError> {
        let media_id = media_id.trim();
        if media_id.is_empty() {
            return Err(MobileApiError::EmptyMediaId);
        }
        if (season == 0) != (episode == 0) {
            return Err(MobileApiError::InvalidEpisodeSelection);
        }

        let requested_provider = Self::parse_provider(provider)?;
        let mut title = media_title_from_id(media_id);
        let mut source_ids: Vec<crate::providers::models::ProviderMediaId> =
            vec![crate::providers::models::ProviderMediaId {
                provider: requested_provider,
                value: media_id.to_string(),
            }];

        // A title may have several MovieBox subject IDs (one per audio cut).
        // Start with the exact selected ID and append sibling cuts before other servers.
        if let Ok(details) = self
            .service
            .details_typed(requested_provider, media_id)
            .await
        {
            title = details.title.clone();
            if requested_provider == ProviderKind::MovieBox {
                for dub in details.dubs {
                    if dub.subject_id.trim().is_empty() {
                        continue;
                    }
                    let source = crate::providers::models::ProviderMediaId {
                        provider: requested_provider,
                        value: dub.subject_id,
                    };
                    if !source_ids.contains(&source) {
                        source_ids.push(source);
                    }
                }
            }
        }

        let mut provider_sources = Vec::new();
        let mut releases: Vec<Release> = Vec::new();
        let mut release_sources: Vec<MobileReleaseSource> = Vec::new();
        let mut seen_release_keys = std::collections::HashSet::new();

        for source_id in source_ids.clone() {
            match self
                .service
                .releases_typed(source_id.provider, &source_id.value, season, episode)
                .await
            {
                Ok(candidate_releases) if !candidate_releases.is_empty() => {
                    if !provider_sources.contains(&source_id) {
                        provider_sources.push(source_id.clone());
                    }
                    append_unique_releases(
                        &mut releases,
                        &mut release_sources,
                        &mut seen_release_keys,
                        source_id.provider,
                        &source_id.value,
                        candidate_releases,
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    log::debug!(
                        "playback source lookup failed for {} / {}: {}",
                        source_id.provider,
                        source_id.value,
                        error
                    );
                }
            }
        }

        // Search every other server even when the first server has a stream.
        // This is what makes audio/quality options additive rather than fallback-only.
        let queries = playback_search_queries(&title);
        for alternate_provider in ProviderKind::ENABLED {
            if alternate_provider == requested_provider {
                continue;
            }
            let mut matching_item = None;
            for query in &queries {
                let results = match self
                    .service
                    .search_typed(alternate_provider, query, 1)
                    .await
                {
                    Ok(results) => results,
                    Err(error) => {
                        log::debug!(
                            "catalog fallback search failed for {} with query {:?}: {}",
                            alternate_provider,
                            query,
                            error
                        );
                        continue;
                    }
                };
                matching_item = results.into_iter().find(|item| {
                    let left = normalize_catalog_title(&item.title);
                    let right = normalize_catalog_title(&title);
                    !left.is_empty()
                        && (left == right
                            || (left.len() >= 5
                                && right.len() >= 5
                                && (left.starts_with(&right) || right.starts_with(&left))))
                });
                if matching_item.is_some() {
                    break;
                }
            }

            if let Some(item) = matching_item {
                match self
                    .service
                    .releases_typed(alternate_provider, &item.id.value, season, episode)
                    .await
                {
                    Ok(candidate_releases) if !candidate_releases.is_empty() => {
                        let source_id = crate::providers::models::ProviderMediaId {
                            provider: alternate_provider,
                            value: item.id.value,
                        };
                        if !provider_sources.contains(&source_id) {
                            provider_sources.push(source_id.clone());
                        }
                        append_unique_releases(
                            &mut releases,
                            &mut release_sources,
                            &mut seen_release_keys,
                            source_id.provider,
                            &source_id.value,
                            candidate_releases,
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::debug!("{} had no usable releases: {}", alternate_provider, error);
                    }
                }
            }
        }

        if releases.is_empty() {
            return Err(MobileApiError::PlaybackProvider {
                provider: requested_provider,
                error: ProviderError::Unavailable(
                    "No stream sources available across enabled providers".to_string(),
                ),
            });
        }

        // Captions are collected from each MovieBox cut that contributed a release.
        let mut subtitles: Vec<SubtitleOption> = Vec::new();
        for source_id in provider_sources
            .iter()
            .filter(|source| source.provider == ProviderKind::MovieBox)
        {
            let Some(resource_id) = releases
                .iter()
                .filter(|release| release.provider == ProviderKind::MovieBox)
                .find_map(|release| release.resource_id.clone())
            else {
                continue;
            };
            let sibling_ids = self
                .details(source_id.provider.cache_key(), &source_id.value)
                .await
                .map(|details| details.sibling_ids())
                .unwrap_or_default();
            if let Ok(found) = self
                .service
                .get_ext_captions(
                    &source_id.value,
                    &resource_id,
                    &sibling_ids,
                    season,
                    episode,
                )
                .await
            {
                for subtitle in found {
                    if !subtitles
                        .iter()
                        .any(|existing| existing.url == subtitle.url)
                    {
                        subtitles.push(subtitle);
                    }
                }
            }
        }

        Ok(MobilePlaybackResponse {
            provider: requested_provider,
            media_id: media_id.to_string(),
            season,
            episode,
            releases,
            subtitles,
            release_sources,
            provider_sources,
        })
    }

    /// Resolve a selected release by index from the current provider response.
    /// This deliberately accepts no arbitrary URL from the client.
    pub async fn resolve_playback(
        &self,
        provider: &str,
        media_id: &str,
        season: usize,
        episode: usize,
        release_index: usize,
        subtitle_index: Option<usize>,
    ) -> Result<PlaybackSource, MobileApiError> {
        let provider = Self::parse_provider(provider)?;
        let releases = self
            .service
            .releases_typed(provider, media_id, season, episode)
            .await
            .map_err(|error| MobileApiError::PlaybackProvider { provider, error })?;
        let release = releases
            .get(release_index)
            .ok_or(MobileApiError::InvalidReleaseIndex)?;
        let mut source = self
            .service
            .resolve_release_typed(provider, release)
            .await
            .map_err(|error| MobileApiError::PlaybackProvider { provider, error })?;

        if let Some(index) = subtitle_index {
            // Subtitles are optional; retrieve the same combined subtitle list
            // only when the user explicitly selected a subtitle.
            let candidates = self
                .playback_sources(provider.cache_key(), media_id, season, episode)
                .await?;
            let subtitle = candidates
                .subtitles
                .get(index)
                .ok_or(MobileApiError::InvalidSubtitleIndex)?;
            let parsed = url::Url::parse(&subtitle.url).map_err(|_| {
                MobileApiError::Provider(ProviderError::Unavailable(
                    "Provider returned an invalid subtitle URL".to_string(),
                ))
            })?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                return Err(MobileApiError::Provider(ProviderError::Unavailable(
                    "Only HTTP and HTTPS subtitle URLs are supported".to_string(),
                )));
            }
            source.subtitle = Some(subtitle.url.clone());
        }

        Ok(source)
    }

    pub async fn homepage(
        &self,
        tab_id: &str,
        page: usize,
    ) -> Result<MobileHomepage, MobileApiError> {
        let tab_id = tab_id.trim();
        if tab_id.is_empty() {
            return Err(MobileApiError::EmptyTabId);
        }

        let (items, metrics) = self
            .service
            .homepage(tab_id, page.clamp(1, 1000))
            .await
            .map_err(MobileApiError::Service)?;

        Ok(MobileHomepage { items, metrics })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_ids_and_aliases() {
        assert_eq!(
            MobileApi::parse_provider("moviebox").unwrap(),
            ProviderKind::MovieBox
        );
        assert_eq!(
            MobileApi::parse_provider("4KHDHub").unwrap(),
            ProviderKind::FourKHdHub
        );
        assert_eq!(
            MobileApi::parse_provider("circleftp (bdix)").unwrap(),
            ProviderKind::BdixCircleFtp
        );
    }

    #[test]
    fn rejects_unknown_provider() {
        let error = MobileApi::parse_provider("not-a-provider").unwrap_err();
        assert!(matches!(error, MobileApiError::UnknownProvider(_)));
    }

    #[tokio::test]
    async fn rejects_blank_search_before_provider_access() {
        let api = MobileApi::new();
        let error = api.search("moviebox", "   ", 1).await.unwrap_err();
        assert!(matches!(error, MobileApiError::EmptyQuery));
    }

    #[tokio::test]
    async fn rejects_blank_media_id_before_provider_access() {
        let api = MobileApi::new();
        let error = api.details("moviebox", "  ").await.unwrap_err();
        assert!(matches!(error, MobileApiError::EmptyMediaId));
    }

    #[tokio::test]
    async fn rejects_partial_episode_selection_before_provider_access() {
        let api = MobileApi::new();
        let error = api
            .playback_sources("moviebox", "valid-id", 1, 0)
            .await
            .unwrap_err();
        assert!(matches!(error, MobileApiError::InvalidEpisodeSelection));
    }
}

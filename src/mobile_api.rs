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
use crate::providers::models::{CatalogItem, MediaDetails, ProviderError, ProviderKind, Release, SubtitleOption};
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
    #[error("Choose either a movie (season and episode both zero) or a specific season and episode")]
    InvalidEpisodeSelection,
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("{0}")]
    Service(String),
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

        let provider = Self::parse_provider(provider)?;
        let releases = self
            .service
            .releases_typed(provider, media_id, season, episode)
            .await
            .map_err(MobileApiError::from)?;

        // Caption lookup is best-effort: a subtitle service failure must not
        // prevent the user from seeing available video releases.
        let subtitles = if provider == ProviderKind::MovieBox {
            let resource_id = releases.iter().find_map(|release| release.resource_id.as_deref());
            if let Some(resource_id) = resource_id {
                let sibling_ids = self
                    .details(provider.cache_key(), media_id)
                    .await
                    .map(|details| details.sibling_ids())
                    .unwrap_or_default();
                self.service
                    .get_ext_captions(media_id, resource_id, &sibling_ids, season, episode)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Ok(MobilePlaybackResponse {
            provider,
            media_id: media_id.to_string(),
            season,
            episode,
            releases,
            subtitles,
        })
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
}

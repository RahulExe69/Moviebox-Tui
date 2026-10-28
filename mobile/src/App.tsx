import { useEffect, useMemo, useState, type FormEvent } from "react";
import {
  apiRequest,
  connectBackend,
  playInNativePlayer,
  resetConnection,
  type CatalogItem,
  type MediaDetails,
  type MobilePlaybackResponse,
  type NativeStreamSource,
  type ProviderOption,
  type ResolvedPlaybackSource,
} from "./api";

type Phase = "connecting" | "ready" | "error";
type ResultFilter = "all" | "movie" | "series";
type Screen = "home" | "explore" | "list" | "settings";

const FAVORITES_KEY = "7xcinema:favorites:v1";
const SCREEN_LABELS: Record<Screen, string> = {
  home: "Home",
  explore: "Explore",
  list: "My List",
  settings: "Settings",
};

function itemKey(item: CatalogItem): string {
  return item.id.provider + ":" + item.id.value;
}

export default function App() {
  const [phase, setPhase] = useState<Phase>("connecting");
  const [providers, setProviders] = useState<ProviderOption[]>([]);
  const [providerId, setProviderId] = useState("all");
  const [screen, setScreen] = useState<Screen>("home");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<CatalogItem[]>([]);
  const [homeItems, setHomeItems] = useState<CatalogItem[]>([]);
  const [homeLoading, setHomeLoading] = useState(false);
  const [homeUnavailable, setHomeUnavailable] = useState(false);
  const [favorites, setFavorites] = useState<CatalogItem[]>([]);
  const [filter, setFilter] = useState<ResultFilter>("all");
  const [searching, setSearching] = useState(false);
  const [details, setDetails] = useState<MediaDetails | null>(null);
  const [playback, setPlayback] = useState<MobilePlaybackResponse | null>(null);
  const [loadingPlayback, setLoadingPlayback] = useState(false);
  const [playingReleaseIndex, setPlayingReleaseIndex] = useState<number | null>(null);
  const [selectedSubtitleIndex, setSelectedSubtitleIndex] = useState("");
  const [playbackError, setPlaybackError] = useState("");
  const [selectedEpisode, setSelectedEpisode] = useState<{ season: number; episode: number } | null>(null);
  const [openingItemKey, setOpeningItemKey] = useState<string | null>(null);
  const [statusText, setStatusText] = useState("Starting on-device engine");
  const [error, setError] = useState("");
  const [bootAttempt, setBootAttempt] = useState(0);

  useEffect(() => {
    try {
      const saved = localStorage.getItem(FAVORITES_KEY);
      if (saved) {
        const parsed: unknown = JSON.parse(saved);
        if (Array.isArray(parsed)) setFavorites(parsed as CatalogItem[]);
      }
    } catch {
      // A damaged local list should not stop the app from opening.
      setFavorites([]);
    }
  }, []);

  useEffect(() => {
    let active = true;
    async function boot() {
      setPhase("connecting");
      setError("");
      setStatusText("Starting on-device engine");
      try {
        await connectBackend();
        const health = await apiRequest<{ ok: boolean }>("/api/v1/health");
        if (!health.ok) throw new Error("Local service health check failed");
        const choices = await apiRequest<ProviderOption[]>("/api/v1/providers");
        if (!active) return;
        setProviders(choices);
        setProviderId("all");
        setPhase("ready");
        setStatusText("On-device engine ready");
      } catch (reason) {
        if (!active) return;
        setPhase("error");
        setStatusText("Engine unavailable");
        setError(reason instanceof Error ? reason.message : "Could not connect to the local service.");
      }
    }
    void boot();
    return () => { active = false; };
  }, [bootAttempt]);

  useEffect(() => {
    if (phase !== "ready") return;
    let active = true;
    setHomeLoading(true);
    setHomeUnavailable(false);
    // The provider already supplies a homepage catalogue. Keep its loading
    // separate from app startup so a feed outage never blocks search.
    void apiRequest<{ items: CatalogItem[] }>("/api/v1/homepage?tab=2&page=1")
      .then((result) => {
        if (!active) return;
        setHomeItems(result.items ?? []);
      })
      .catch(() => {
        if (!active) return;
        setHomeItems([]);
        setHomeUnavailable(true);
      })
      .finally(() => {
        if (active) setHomeLoading(false);
      });
    return () => { active = false; };
  }, [phase, bootAttempt]);

  const visibleItems = useMemo(
    () => filter === "all" ? items : items.filter((item) => item.media_type === filter),
    [filter, items]
  );
  const availableProviders = useMemo(
    () => providers.filter((provider) => provider.available),
    [providers]
  );

  function toggleFavorite(item: CatalogItem) {
    setFavorites((current) => {
      const exists = current.some((favorite) => itemKey(favorite) === itemKey(item));
      const next = exists
        ? current.filter((favorite) => itemKey(favorite) !== itemKey(item))
        : [item, ...current];
      try { localStorage.setItem(FAVORITES_KEY, JSON.stringify(next)); } catch { /* Storage may be unavailable. */ }
      return next;
    });
  }

  function isFavorite(item: CatalogItem): boolean {
    return favorites.some((favorite) => itemKey(favorite) === itemKey(item));
  }

  async function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const cleanQuery = query.trim();
    if (!cleanQuery || phase !== "ready" || searching) return;
    setScreen("explore");
    setSearching(true);
    setError("");
    setDetails(null);
    setPlayback(null);
    setPlaybackError("");
    setFilter("all");
    try {
      const results = await apiRequest<CatalogItem[]>("/api/v1/search", {
        method: "POST",
        body: JSON.stringify({ provider: providerId, query: cleanQuery, page: 1 })
      });
      setItems(results);
      setStatusText(results.length ? results.length + " merged titles across servers" : "No results across enabled sources");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Search failed. Please try again.");
      setItems([]);
    } finally {
      setSearching(false);
    }
  }

  async function openDetails(item: CatalogItem) {
    setError("");
    setPlayback(null);
    setSelectedEpisode(null);
    setPlaybackError("");
    try {
      const result = await apiRequest<MediaDetails>("/api/v1/details", {
        method: "POST",
        body: JSON.stringify({ provider: item.id.provider, media_id: item.id.value })
      });
      setDetails(result);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not load title details.");
    }
  }

  async function findPlaybackSources() {
    if (!details || loadingPlayback) return;
    setLoadingPlayback(true);
    setPlaybackError("");
    setPlayback(null);
    const firstEpisode = details.seasons.flatMap((season) => season.episodes)[0];
    const season = details.media_type === "series"
      ? Math.max(1, firstEpisode?.season ?? details.seasons[0]?.number ?? 1)
      : 0;
    const episode = details.media_type === "series"
      ? Math.max(1, firstEpisode?.number ?? 1)
      : 0;
    try {
      const result = await apiRequest<MobilePlaybackResponse>("/api/v1/playback/sources", {
        method: "POST",
        body: JSON.stringify({
          provider: details.id.provider,
          media_id: details.id.value,
          season,
          episode
        })
      });
      setPlayback(result);
      setSelectedSubtitleIndex(result.subtitles.length > 0 ? "0" : "");
    } catch (reason) {
      setPlaybackError(reason instanceof Error ? reason.message : "Could not find playback sources.");
    } finally {
      setLoadingPlayback(false);
    }
  }

  async function launchPlayback(
    targetDetails: MediaDetails,
    seasonOverride?: number,
    episodeOverride?: number,
    useSelectedSubtitle = true
  ) {
    if (phase !== "ready") return;
    setLoadingPlayback(true);
    setPlaybackError("");
    const firstEpisode = targetDetails.seasons.flatMap((season) => season.episodes)[0];
    const season = targetDetails.media_type === "series"
      ? Math.max(1, seasonOverride ?? firstEpisode?.season ?? targetDetails.seasons[0]?.number ?? 1)
      : 0;
    const episode = targetDetails.media_type === "series"
      ? Math.max(1, episodeOverride ?? firstEpisode?.number ?? 1)
      : 0;
    setSelectedEpisode(targetDetails.media_type === "series" ? { season, episode } : null);

    try {
      const candidates = await apiRequest<MobilePlaybackResponse>("/api/v1/playback/sources", {
        method: "POST",
        body: JSON.stringify({
          provider: targetDetails.id.provider,
          media_id: targetDetails.id.value,
          season,
          episode
        })
      });
      setPlayback(candidates);
      if (!candidates.releases.length) {
        throw new Error("No playable releases were returned by the enabled providers.");
      }

      const subtitleIndex = useSelectedSubtitle && selectedSubtitleIndex !== ""
        ? Number(selectedSubtitleIndex)
        : undefined;
      // Prioritise one candidate per provider first, then one per individual source
      // (including alternate audio IDs), then quality/mirror alternatives.
      const orderedIndexes: number[] = [];
      const usedIndexes = new Set<number>();
      const firstByProvider = new Map<string, number>();
      const firstBySource = new Map<string, number>();
      (candidates.release_sources ?? []).forEach((sourceRef, index) => {
        if (!firstByProvider.has(sourceRef.provider)) firstByProvider.set(sourceRef.provider, index);
        const key = sourceRef.provider + ":" + sourceRef.media_id;
        if (!firstBySource.has(key)) firstBySource.set(key, index);
      });
      for (const index of [...firstByProvider.values(), ...firstBySource.values()]) {
        if (!usedIndexes.has(index)) { orderedIndexes.push(index); usedIndexes.add(index); }
      }
      candidates.releases.forEach((_release, index) => {
        if (!usedIndexes.has(index)) { orderedIndexes.push(index); usedIndexes.add(index); }
      });

      const failures: string[] = [];
      const resolvedSources: NativeStreamSource[] = [];
      const maxResolvedSources = 12;
      for (const index of orderedIndexes) {
        if (resolvedSources.length >= maxResolvedSources) break;
        const sourceRef = candidates.release_sources?.[index];
        try {
          const source = await apiRequest<ResolvedPlaybackSource>("/api/v1/playback/resolve", {
            method: "POST",
            body: JSON.stringify({
              provider: sourceRef?.provider ?? candidates.provider,
              media_id: sourceRef?.media_id ?? candidates.media_id,
              season: candidates.season,
              episode: candidates.episode,
              release_index: sourceRef?.release_index ?? index,
              ...(subtitleIndex === undefined ? {} : { subtitle_index: subtitleIndex })
            })
          });
          resolvedSources.push({
            url: source.url,
            headers: Object.fromEntries(source.headers),
            label: [
              source.provider,
              source.source_label || candidates.releases[index]?.filename || "Stream source"
            ].filter(Boolean).join(" · "),
            ...(source.subtitle ? { subtitleUrl: source.subtitle } : {})
          });
        } catch (reason) {
          failures.push(reason instanceof Error ? reason.message : "Source could not be resolved");
        }
      }

      if (!resolvedSources.length) {
        throw new Error(failures[0] || "No source could be resolved. Try another title or provider.");
      }

      // Remove the details sheet only when a real player launch is about to happen.
      setDetails(null);
      await playInNativePlayer({
        url: resolvedSources[0].url,
        title: targetDetails.title,
        headers: resolvedSources[0].headers,
        ...(resolvedSources[0].subtitleUrl ? { subtitleUrl: resolvedSources[0].subtitleUrl } : {}),
        sources: resolvedSources
      });
    } catch (reason) {
      setDetails(targetDetails);
      setPlaybackError(reason instanceof Error ? reason.message : "Could not start playback.");
    } finally {
      setLoadingPlayback(false);
    }
  }

  async function watchNow(seasonOverride?: number, episodeOverride?: number) {
    if (!details || loadingPlayback || phase !== "ready") return;
    await launchPlayback(details, seasonOverride, episodeOverride, true);
  }

  async function startCatalogItem(item: CatalogItem) {
    if (phase !== "ready" || loadingPlayback || openingItemKey) return;
    setOpeningItemKey(itemKey(item));
    setDetails(null);
    setPlayback(null);
    setPlaybackError("");
    setError("");
    try {
      const target = await apiRequest<MediaDetails>("/api/v1/details", {
        method: "POST",
        body: JSON.stringify({ provider: item.id.provider, media_id: item.id.value })
      });
      // Like 7xAnime, tapping the poster opens playback directly. A series starts
      // with its first available episode (or S1E1 when metadata is incomplete).
      const firstEpisode = target.seasons.flatMap((season) => season.episodes)[0];
      const season = target.media_type === "series"
        ? Math.max(1, firstEpisode?.season ?? target.seasons[0]?.number ?? 1)
        : 0;
      const episode = target.media_type === "series"
        ? Math.max(1, firstEpisode?.number ?? 1)
        : 0;
      await launchPlayback(target, season, episode, false);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Could not start this title.");
    } finally {
      setOpeningItemKey(null);
    }
  }

  async function playRelease(releaseIndex: number) {
    if (!details || !playback || playingReleaseIndex !== null) return;
    setPlayingReleaseIndex(releaseIndex);
    setPlaybackError("");
    const subtitleIndex = selectedSubtitleIndex === "" ? undefined : Number(selectedSubtitleIndex);
    try {
      const source = await apiRequest<ResolvedPlaybackSource>("/api/v1/playback/resolve", {
        method: "POST",
        body: JSON.stringify({
          provider: playback.release_sources?.[releaseIndex]?.provider ?? playback.provider,
          media_id: playback.release_sources?.[releaseIndex]?.media_id ?? playback.media_id,
          season: playback.season,
          episode: playback.episode,
          release_index: playback.release_sources?.[releaseIndex]?.release_index ?? releaseIndex,
          ...(subtitleIndex === undefined ? {} : { subtitle_index: subtitleIndex })
        })
      });
      const headers = Object.fromEntries(source.headers);
      await playInNativePlayer({
        url: source.url,
        title: details.title,
        headers,
        ...(source.subtitle ? { subtitleUrl: source.subtitle } : {})
      });
    } catch (reason) {
      setPlaybackError(reason instanceof Error ? reason.message : "Could not open this playback source.");
    } finally {
      setPlayingReleaseIndex(null);
    }
  }

  function retryConnection() {
    resetConnection();
    setBootAttempt((attempt) => attempt + 1);
  }

  function renderCard(item: CatalogItem) {
    const saved = isFavorite(item);
    return (
      <article className="media-card" key={itemKey(item)}>
        <button className="poster-button" type="button" onClick={() => void startCatalogItem(item)} disabled={openingItemKey !== null || loadingPlayback} aria-label={"Play " + item.title}>
          <div className="poster-art">
            {item.poster_url
              ? <img src={item.poster_url} loading="lazy" alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} />
              : null}
            <div className="poster-placeholder"><span>7x</span><small>CINEMA</small></div>
            <div className="poster-shade" />
            <span className="poster-type">{item.media_type === "series" ? "SERIES" : "MOVIE"}</span>
            {item.year && <span className="poster-year">{item.year}</span>}
            <span className="poster-play" aria-hidden="true">{openingItemKey === itemKey(item) ? "…" : "▶"}</span>
            <div className="poster-caption">
              <strong>{item.title}</strong>
              <span>{item.languages?.length ? item.languages.join(" · ") : item.provider_count && item.provider_count > 1 ? item.provider_count + " servers" : item.season_count ? item.season_count + " seasons" : item.year || "Discover"}</span>
            </div>
          </div>
        </button>
        <button type="button" className="poster-info-button" aria-label={"Details for " + item.title} onClick={() => void openDetails(item)}>ⓘ</button>
        <button
          type="button"
          className={"favorite-toggle" + (saved ? " is-saved" : "")}
          aria-label={saved ? "Remove from My List" : "Add to My List"}
          aria-pressed={saved}
          onClick={() => toggleFavorite(item)}
        >{saved ? "♥" : "♡"}</button>
      </article>
    );
  }

  const featured = homeItems[0] ?? null;
  const isSavedFeatured = featured ? isFavorite(featured) : false;
  const homeMovies = useMemo(() => homeItems.filter((item) => item.media_type === "movie"), [homeItems]);
  const homeSeries = useMemo(() => homeItems.filter((item) => item.media_type === "series"), [homeItems]);

  return (
    <div className="app-shell">
      <header className="topbar">
        <button className="brand" type="button" onClick={() => setScreen("home")} aria-label="7xCinema home">
          <span className="brand-mark"><img src="/brand/7xcinema-icon.png" alt="" /></span>
          <span className="brand-name"><img src="/brand/7xcinema-wordmark.png" alt="7xCinema" /></span>
        </button>
        <nav className="desktop-nav" aria-label="Primary navigation">
          {(["home", "explore", "list", "settings"] as Screen[]).map((destination) => (
            <button key={destination} type="button" className={screen === destination ? "nav-link active" : "nav-link"} onClick={() => setScreen(destination)}>
              {SCREEN_LABELS[destination]}
            </button>
          ))}
        </nav>
        <div className={"engine-status " + phase} title={statusText}><span className="status-dot" /><span>{phase === "ready" ? "Engine ready" : phase === "error" ? "Offline" : "Starting"}</span></div>
      </header>

      <main className="app-main">
        {phase === "error" && (
          <div className="connection-error">
            <div><strong>Couldn't connect just yet</strong><span>{error}</span></div>
            <button type="button" onClick={retryConnection}>Try again</button>
          </div>
        )}

        {screen === "home" && (
          <section className="home-screen">
            <div className="home-heading">
              <div className="eyebrow"><span /> YOUR NEXT MOVIE NIGHT</div>
              <h1>Stories worth <em>staying up for.</em></h1>
              <p>Movies, series and anime—together in your cinema.</p>
            </div>

            {featured ? (
              <section className="featured-hero" aria-label="Featured title">
                {featured.poster_url && <img className="featured-image" src={featured.poster_url} alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} />}
                <div className="featured-overlay" />
                <div className="featured-content">
                  <span className="featured-kicker"><i /> 7XCINEMA SPOTLIGHT</span>
                  <span className="featured-type">{featured.media_type === "series" ? "FEATURED SERIES" : "FEATURED MOVIE"}{featured.year ? " · " + featured.year : ""}</span>
                  <h2>{featured.title}</h2>
                  <p>Picked from your connected catalogue. Open the title to see details and available playback options.</p>
                  <div className="featured-actions">
                    <button className="primary-button" type="button" onClick={() => void startCatalogItem(featured)}><span>▶</span> Watch now</button>
                    <button className={"secondary-button" + (isSavedFeatured ? " is-saved" : "")} type="button" onClick={() => toggleFavorite(featured)}>{isSavedFeatured ? "♥ Added to My List" : "♡ Add to My List"}</button>
                  </div>
                </div>
                <div className="featured-count"><span>01</span><i /> SPOTLIGHT</div>
              </section>
            ) : (
              <section className="featured-hero featured-empty">
                <div className="featured-orb orb-one" /><div className="featured-orb orb-two" />
                <div className="featured-content">
                  <span className="featured-kicker"><i /> YOUR PERSONAL CINEMA</span>
                  <h2>{homeLoading ? "Finding your next watch…" : "Your next favourite is waiting."}</h2>
                  <p>{homeUnavailable ? "The home feed isn't available right now, but search still works." : "Explore connected sources and build a list of the titles you want to watch."}</p>
                  <div className="featured-actions">
                    <button className="primary-button" type="button" onClick={() => setScreen("explore")}>Explore titles <span>↗</span></button>
                    <button className="secondary-button" type="button" onClick={() => void retryConnection()}>Refresh engine</button>
                  </div>
                </div>
              </section>
            )}

            <div className="quick-actions">
              <button type="button" onClick={() => { setItems(homeMovies.length ? homeMovies : items); setFilter("movie"); setScreen("explore"); }}><span className="quick-icon">▣</span><span><b>Movies</b><small>Feature films</small></span><span className="quick-arrow">↗</span></button>
              <button type="button" onClick={() => { setItems(homeSeries.length ? homeSeries : items); setFilter("series"); setScreen("explore"); }}><span className="quick-icon">▤</span><span><b>Series & anime</b><small>Episode by episode</small></span><span className="quick-arrow">↗</span></button>
              <button type="button" onClick={() => setScreen("list")}><span className="quick-icon">♡</span><span><b>My List</b><small>{favorites.length} saved titles</small></span><span className="quick-arrow">↗</span></button>
            </div>

            <section className="shelf-section">
              <div className="shelf-heading">
                <div><span className="section-eyebrow">CURATED HOME FEED</span><h2>Recommended picks</h2></div>
                <button type="button" className="text-link" onClick={() => setScreen("explore")}>Explore all <span>↗</span></button>
              </div>
              {homeLoading && homeItems.length === 0
                ? <div className="loading-shelf"><span className="spinner" /> Loading the catalogue…</div>
                : homeItems.length > 0
                  ? <div className="media-shelf">{homeItems.slice(1, 13).map(renderCard)}</div>
                  : <div className="shelf-empty"><span>✦</span><p>{homeUnavailable ? "The catalogue feed could not load. You can still search any connected source." : "Your catalogue will appear here."}</p><button type="button" onClick={() => setScreen("explore")}>Start exploring</button></div>}
            </section>

            {homeMovies.length > 0 && (
              <section className="shelf-section">
                <div className="shelf-heading">
                  <div><span className="section-eyebrow">FEATURE FILMS</span><h2>Movies</h2></div>
                  <button type="button" className="text-link" onClick={() => { setItems(homeMovies); setFilter("movie"); setScreen("explore"); }}>View all <span>↗</span></button>
                </div>
                <div className="media-shelf">{homeMovies.slice(0, 12).map(renderCard)}</div>
              </section>
            )}

            {homeSeries.length > 0 && (
              <section className="shelf-section">
                <div className="shelf-heading">
                  <div><span className="section-eyebrow">BINGE-WORTHY STORIES</span><h2>Series & anime</h2></div>
                  <button type="button" className="text-link" onClick={() => { setItems(homeSeries); setFilter("series"); setScreen("explore"); }}>View all <span>↗</span></button>
                </div>
                <div className="media-shelf">{homeSeries.slice(0, 12).map(renderCard)}</div>
              </section>
            )}

            {favorites.length > 0 && (
              <section className="shelf-section">
                <div className="shelf-heading">
                  <div><span className="section-eyebrow">SAVED FOR LATER</span><h2>My List</h2></div>
                  <button type="button" className="text-link" onClick={() => setScreen("list")}>View list <span>↗</span></button>
                </div>
                <div className="media-shelf">{favorites.slice(0, 10).map(renderCard)}</div>
              </section>
            )}
          </section>
        )}

        {screen === "explore" && (
          <section className="explore-screen page-screen">
            <div className="page-heading">
              <div className="eyebrow"><span /> FIND YOUR NEXT WATCH</div>
              <h1>Explore <em>the catalogue.</em></h1>
              <p>Search by title across your connected providers.</p>
            </div>
            <section className="search-panel" aria-label="Search titles">
              <form className="search-form" onSubmit={submitSearch}>
                <label className="search-input-wrap"><span className="search-symbol">⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search movies, series or anime…" aria-label="Search titles" autoComplete="off" /></label>
                <label className="provider-select-wrap"><span className="provider-label">SEARCH</span><select value={providerId} onChange={(event) => setProviderId(event.target.value)} aria-label="Search source" disabled={phase !== "ready" || providers.length === 0}><option value="all">All servers · merged</option>{providers.map((provider) => <option key={provider.id} value={provider.id} disabled={!provider.available}>{provider.name}{provider.available ? "" : " · unavailable"}</option>)}</select></label>
                <button className="search-button" type="submit" disabled={phase !== "ready" || searching || !query.trim()}>{searching ? <span className="spinner" /> : "Search"}{!searching && <span aria-hidden="true"> ↗</span>}</button>
              </form>
              {phase === "connecting" && <div className="inline-note"><span className="spinner" /> Starting your on-device source engine…</div>}
              {phase === "ready" && <div className="source-row"><span className="source-check">✓</span><span>{availableProviders.length} sources searched in parallel</span><span className="source-separator">•</span><span>Local-first browsing</span></div>}
            </section>
            {error && phase === "ready" && <div className="error-inline">{error}</div>}
            <div className="results-heading">
              <div><span className="section-eyebrow">{items.length ? "SEARCH RESULTS" : "READY WHEN YOU ARE"}</span><h2>{items.length ? "Choose your next watch" : "What are we in the mood for?"}</h2></div>
              {items.length > 0 && <div className="filter-group" aria-label="Filter results"><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")} type="button">All</button><button className={filter === "movie" ? "active" : ""} onClick={() => setFilter("movie")} type="button">Movies</button><button className={filter === "series" ? "active" : ""} onClick={() => setFilter("series")} type="button">Series</button></div>}
            </div>
            {visibleItems.length > 0
              ? <div className="title-grid">{visibleItems.map(renderCard)}</div>
              : <div className="empty-state"><div className="empty-icon">✳</div><h3>{items.length ? "Nothing in this category" : searching ? "Searching…" : "A world of stories, one search away"}</h3><p>{items.length ? "Try another filter to see more results." : "Enter a title above to search all enabled servers and merge matching titles."}</p>{!searching && !items.length && <button className="secondary-button" type="button" onClick={() => setScreen("home")}>Back to Home</button>}</div>}
          </section>
        )}

        {screen === "list" && (
          <section className="page-screen list-screen">
            <div className="page-heading">
              <div className="eyebrow"><span /> YOUR PERSONAL COLLECTION</div>
              <h1>My <em>List.</em></h1>
              <p>Keep your next watch close. Your list is saved on this device.</p>
            </div>
            {favorites.length > 0
              ? <div className="title-grid">{favorites.map(renderCard)}</div>
              : <div className="empty-state list-empty"><div className="empty-icon">♡</div><h3>Your list starts here</h3><p>Tap the heart on any movie or series to save it here for later.</p><button className="primary-button" type="button" onClick={() => setScreen("explore")}>Explore titles <span>↗</span></button></div>}
          </section>
        )}

        {screen === "settings" && (
          <section className="page-screen settings-screen">
            <div className="page-heading">
              <div className="eyebrow"><span /> MAKE IT YOURS</div>
              <h1>App <em>settings.</em></h1>
              <p>Manage your source preference and on-device engine.</p>
            </div>
            <section className="settings-group">
              <div className="settings-group-title"><span className="settings-icon">◈</span><div><h2>Preferred source</h2><p>Used when you open Explore.</p></div></div>
              <label className="settings-select"><span>SEARCH PROVIDER</span><select value={providerId} onChange={(event) => setProviderId(event.target.value)} disabled={phase !== "ready"}><option value="all">All servers · merged</option>{providers.map((provider) => <option key={provider.id} value={provider.id} disabled={!provider.available}>{provider.name}{provider.available ? "" : " · unavailable"}</option>)}</select></label>
              <div className="provider-list">{providers.map((provider) => <div className="provider-row" key={provider.id}><span className={"provider-status" + (provider.available ? " online" : "")} /><span className="provider-name">{provider.name}</span><span className="provider-state">{provider.available ? "Available" : "Unavailable"}</span></div>)}</div>
            </section>
            <section className="settings-group">
              <div className="settings-group-title"><span className="settings-icon">⌘</span><div><h2>On-device engine</h2><p>Your provider requests run locally on this device.</p></div></div>
              <div className="engine-detail"><span className={"engine-pill " + phase}><i />{phase === "ready" ? "Connected" : phase === "connecting" ? "Connecting" : "Disconnected"}</span><button className="secondary-button" type="button" onClick={retryConnection}>Restart engine</button></div>
            </section>
            <div className="settings-footer"><span className="brand-mark small-mark"><img src="/brand/7xcinema-icon.png" alt="" /></span><div><strong><img className="settings-wordmark" src="/brand/7xcinema-wordmark.png" alt="7xCinema" /></strong><span>Built for your next watch.</span></div><span className="version-label">ANDROID · ARM64</span></div>
          </section>
        )}
        <footer className="app-footer"><span className="footer-logo">7xCinema</span><span>Local-first by design</span><span className="footer-dot">•</span><span>Source availability can vary</span></footer>
      </main>

      <nav className="bottom-nav" aria-label="Mobile navigation">
        {([
          ["home", "⌂"],
          ["explore", "⌕"],
          ["list", "♡"],
          ["settings", "⚙"],
        ] as [Screen, string][]).map(([destination, icon]) => (
          <button key={destination} type="button" className={screen === destination ? "bottom-nav-item active" : "bottom-nav-item"} aria-current={screen === destination ? "page" : undefined} onClick={() => setScreen(destination)}>
            <span className="bottom-nav-icon" aria-hidden="true">{icon}</span><span>{SCREEN_LABELS[destination]}</span>
            {screen === destination && <i className="nav-active-mark" />}
          </button>
        ))}
      </nav>

      {details && <div className="modal-backdrop" role="presentation" onClick={() => setDetails(null)}><section className="details-modal" role="dialog" aria-modal="true" aria-label={details.title} onClick={(event) => event.stopPropagation()}>
        <button className="modal-close" type="button" onClick={() => setDetails(null)} aria-label="Close details">×</button>
        <div className="details-poster">{details.poster_url ? <img src={details.poster_url} alt="" /> : <div className="poster-fallback"><span>7x</span><small>CINEMA</small></div>}</div>
        <div className="details-copy"><div className="section-kicker">{details.media_type === "series" ? "SERIES DETAILS" : "MOVIE DETAILS"}</div><h2>{details.title}</h2><div className="details-meta">{details.year && <span>{details.year}</span>}{details.duration && <span>{details.duration}</span>}{details.imdb_rating && <span>★ {details.imdb_rating}</span>}</div>
          {details.tagline && <p className="tagline">{details.tagline}</p>}<p className="description">{details.description || "No description was provided by this source."}</p>
          {details.genres.length > 0 && <div className="genre-list">{details.genres.map((genre) => <span key={genre}>{genre}</span>)}</div>}
          {details.director && <p className="credit"><b>Director</b> {details.director}</p>}{details.stars && <p className="credit"><b>Cast</b> {details.stars}</p>}
          {details.seasons.length > 0 && <section className="episode-picker" aria-label="Episodes">
            <div className="episode-picker-heading"><div><span className="section-kicker">EPISODES</span><h3>Choose an episode</h3></div><span>{details.seasons.reduce((total, season) => total + season.episodes.length, 0)} available</span></div>
            {details.seasons.map((season) => <div className="season-group" key={season.number}>
              <h4>Season {season.number}</h4>
              <div className="episode-list">{season.episodes.map((episode) => {
                const active = selectedEpisode?.season === season.number && selectedEpisode?.episode === episode.number;
                return <button className={"episode-card" + (active ? " active" : "")} type="button" key={season.number + ":" + episode.number} disabled={loadingPlayback} onClick={() => void watchNow(season.number, episode.number)}>
                  <span className="episode-number">{String(episode.number).padStart(2, "0")}</span>
                  <span className="episode-copy"><strong>{episode.title || "Episode " + episode.number}</strong>{episode.overview && <small>{episode.overview}</small>}</span>
                  <span className="episode-play">{active && loadingPlayback ? "…" : "▶"}</span>
                </button>;
              })}</div>
            </div>)}
          </section>}
          {details.dubs.length > 0 && <p className="credit"><b>Audio options</b> {details.dubs.map((dub) => dub.label).join(", ")}</p>}
          <div className="playback-actions">
            <button className="primary-button watch-now-button" type="button" onClick={() => void watchNow()} disabled={loadingPlayback || phase !== "ready"}>
              {loadingPlayback ? <><span className="spinner" /> Finding a playable source…</> : <><span>▶</span> Watch now</>}
            </button>
            <button className="secondary-button source-options-button" type="button" onClick={() => void findPlaybackSources()} disabled={loadingPlayback || phase !== "ready"}>
              {playback ? "Change source / subtitles" : "Other sources & subtitles"}
            </button>
          </div>
          {playbackError && <p className="playback-error" role="alert">{playbackError}</p>}
          {playback && <section className="playback-results" aria-label="Playback sources">
            <div className="playback-results-heading"><strong>STREAM SOURCES</strong><span>{playback.releases.length} found</span></div>
            {playback.subtitles.length > 0 && <label className="subtitle-picker"><span>SUBTITLES</span><select value={selectedSubtitleIndex} onChange={(event) => setSelectedSubtitleIndex(event.target.value)}><option value="">Off</option>{playback.subtitles.map((subtitle, index) => <option key={subtitle.url + index} value={String(index)}>{subtitle.name}</option>)}</select></label>}
            {playback.releases.map((release, index) => <article className="playback-release" key={release.provider + ":" + release.filename + ":" + index}>
              <div className="release-main"><strong>{release.filename || "Stream source"}</strong><span>{providers.find((provider) => provider.id === release.provider)?.name || release.provider}{release.quality ? " · " + release.quality : ""}{release.language ? " · " + release.language : ""}{release.size_bytes ? " · " + (release.size_bytes / 1073741824).toFixed(2) + " GB" : ""}</span></div>
              <div className="release-mirrors">{release.mirrors.length} mirror{release.mirrors.length === 1 ? "" : "s"}{release.mirrors[0] ? " · " + release.mirrors.map((mirror) => mirror.label).filter(Boolean).join(" / ") : ""}</div>
              {release.mirrors.some((mirror) => mirror.headers.length > 0) && <span className="release-header-note">Request headers included</span>}
              <button className="release-play-button" type="button" disabled={playingReleaseIndex !== null} onClick={() => void playRelease(index)}>{playingReleaseIndex === index ? <><span className="spinner" /> Opening player…</> : "▶ Play in app"}</button>
            </article>)}
            <p className="playback-next-step">Choose a source to open the native player. Some provider links may still fail due to upstream restrictions.</p>
          </section>}
          <p className="provider-note">{playback?.provider_sources?.length ? "Merged playback options from " + [...new Set(playback.provider_sources.map((source) => providers.find((provider) => provider.id === source.provider)?.name || source.provider))].join(", ") : "Details from " + (providers.find((provider) => provider.id === details.id.provider)?.name || details.id.provider) + ". Playback checks all enabled providers."}</p>
        </div>
      </section></div>}
    </div>
  );
}

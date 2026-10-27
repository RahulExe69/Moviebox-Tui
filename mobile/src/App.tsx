import { useEffect, useMemo, useState, type FormEvent } from "react";
import { apiRequest, connectBackend, resetConnection, type CatalogItem, type MediaDetails, type ProviderOption } from "./api";

type Phase = "connecting" | "ready" | "error";
type ResultFilter = "all" | "movie" | "series";

export default function App() {
  const [phase, setPhase] = useState<Phase>("connecting");
  const [providers, setProviders] = useState<ProviderOption[]>([]);
  const [providerId, setProviderId] = useState("moviebox");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<CatalogItem[]>([]);
  const [filter, setFilter] = useState<ResultFilter>("all");
  const [searching, setSearching] = useState(false);
  const [details, setDetails] = useState<MediaDetails | null>(null);
  const [statusText, setStatusText] = useState("Starting on-device engine");
  const [error, setError] = useState("");
  const [bootAttempt, setBootAttempt] = useState(0);

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
        const preferred = choices.find((item) => item.id === "moviebox" && item.available) ?? choices.find((item) => item.available);
        if (preferred) setProviderId(preferred.id);
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

  const visibleItems = useMemo(
    () => filter === "all" ? items : items.filter((item) => item.media_type === filter),
    [filter, items]
  );

  async function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const cleanQuery = query.trim();
    if (!cleanQuery || phase !== "ready" || searching) return;
    setSearching(true);
    setError("");
    setDetails(null);
    setFilter("all");
    try {
      const results = await apiRequest<CatalogItem[]>("/api/v1/search", {
        method: "POST",
        body: JSON.stringify({ provider: providerId, query: cleanQuery, page: 1 })
      });
      setItems(results);
      setStatusText(results.length ? results.length + " results found" : "No results on this source");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Search failed. Please try again.");
      setItems([]);
    } finally {
      setSearching(false);
    }
  }

  async function openDetails(item: CatalogItem) {
    setError("");
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

  function retryConnection() {
    resetConnection();
    setBootAttempt((attempt) => attempt + 1);
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <a className="brand" href="#" aria-label="7xCinema home"><span className="brand-mark">7<span>x</span></span><span className="brand-name">7xCinema</span></a>
        <div className={"engine-status " + phase}><span className="status-dot" /><span>{statusText}</span></div>
      </header>
      <main>
        <section className="hero">
          <div className="hero-copy">
            <div className="eyebrow"><span /> YOUR NEXT WATCH STARTS HERE</div>
            <h1>Find the story.<br /><em>Make it a night.</em></h1>
            <p>Search your connected sources in one clean, local-first cinema experience.</p>
          </div>
          <div className="hero-art" aria-hidden="true"><div className="art-glow" /><div className="art-screen"><div className="screen-line" /><div className="screen-play">▶</div><div className="screen-foot"><span>7X</span><small>CINEMA</small></div></div><div className="art-disc" /></div>
        </section>
        <section className="search-panel" aria-label="Search titles">
          <div className="search-heading"><div><div className="section-kicker">DISCOVER</div><h2>What are we watching?</h2></div><div className="local-badge">⌑ &nbsp; LOCAL ENGINE</div></div>
          <form className="search-form" onSubmit={submitSearch}>
            <label className="search-input-wrap"><span className="search-symbol">⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search movies, series, or a title..." aria-label="Search titles" autoComplete="off" /></label>
            <label className="provider-select-wrap"><span className="provider-label">SOURCE</span><select value={providerId} onChange={(event) => setProviderId(event.target.value)} aria-label="Search source" disabled={phase !== "ready" || providers.length === 0}>{providers.map((provider) => <option key={provider.id} value={provider.id} disabled={!provider.available}>{provider.name}{provider.available ? "" : " · unavailable"}</option>)}</select></label>
            <button className="search-button" type="submit" disabled={phase !== "ready" || searching || !query.trim()}>{searching ? <span className="spinner" /> : "Search"}{!searching && <span aria-hidden="true"> ↗</span>}</button>
          </form>
          {phase === "connecting" && <div className="inline-note"><span className="spinner" /> Starting your on-device source engine…</div>}
          {phase === "error" && <div className="error-panel"><div><strong>Couldn’t connect just yet.</strong><span>{error}</span></div><button type="button" onClick={retryConnection}>Try again</button></div>}
          {phase === "ready" && <div className="source-row"><span className="source-check">✓</span><span>{providers.filter((provider) => provider.available).length} sources configured</span><span className="source-separator">•</span><span>Requests stay on your device</span></div>}
        </section>
        <section className="results-section">
          <div className="results-heading"><div><div className="section-kicker">{items.length ? "YOUR RESULTS" : "READY WHEN YOU ARE"}</div><h2>{items.length ? "Choose your next watch" : "A world of stories, one search away"}</h2></div>
            {items.length > 0 && <div className="filter-group" aria-label="Filter results"><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")} type="button">All</button><button className={filter === "movie" ? "active" : ""} onClick={() => setFilter("movie")} type="button">Movies</button><button className={filter === "series" ? "active" : ""} onClick={() => setFilter("series")} type="button">Series</button></div>}
          </div>
          {error && phase === "ready" && <div className="error-inline">{error}</div>}
          {visibleItems.length ? <div className="title-grid">{visibleItems.map((item) => <button className="title-card" key={item.id.provider + ":" + item.id.value} type="button" onClick={() => void openDetails(item)} aria-label={"Show details for " + item.title}>
            <div className="poster">{item.poster_url ? <img src={item.poster_url} loading="lazy" alt="" onError={(event) => { event.currentTarget.style.display = "none"; }} /> : null}<div className="poster-fallback"><span>7x</span><small>CINEMA</small></div><span className="media-pill">{item.media_type === "series" ? "SERIES" : "MOVIE"}</span><span className="poster-open">↗</span></div>
            <div className="card-meta"><strong>{item.title}</strong><span>{item.year || "Year not listed"}{item.season_count ? " · " + item.season_count + " seasons" : ""}</span></div>
          </button>)}</div> : <div className="empty-state"><div className="empty-icon">✳</div><h3>{items.length ? "Nothing in this category" : "Your next favourite is out there"}</h3><p>{items.length ? "Try another filter to see more results." : "Enter a title above and 7xCinema will search the selected source from your device."}</p></div>}
        </section>
        <footer className="footer"><span className="footer-logo">7xCinema</span><span>Local-first by design</span><span className="footer-dot">•</span><span>Source availability can vary</span></footer>
      </main>
      {details && <div className="modal-backdrop" role="presentation" onClick={() => setDetails(null)}><section className="details-modal" role="dialog" aria-modal="true" aria-label={details.title} onClick={(event) => event.stopPropagation()}>
        <button className="modal-close" type="button" onClick={() => setDetails(null)} aria-label="Close details">×</button>
        <div className="details-poster">{details.poster_url ? <img src={details.poster_url} alt="" /> : <div className="poster-fallback"><span>7x</span><small>CINEMA</small></div>}</div>
        <div className="details-copy"><div className="section-kicker">{details.media_type === "series" ? "SERIES DETAILS" : "MOVIE DETAILS"}</div><h2>{details.title}</h2><div className="details-meta">{details.year && <span>{details.year}</span>}{details.duration && <span>{details.duration}</span>}{details.imdb_rating && <span>★ {details.imdb_rating}</span>}</div>
          {details.tagline && <p className="tagline">{details.tagline}</p>}<p className="description">{details.description || "No description was provided by this source."}</p>
          {details.genres.length > 0 && <div className="genre-list">{details.genres.map((genre) => <span key={genre}>{genre}</span>)}</div>}
          {details.director && <p className="credit"><b>Director</b> {details.director}</p>}{details.stars && <p className="credit"><b>Cast</b> {details.stars}</p>}
          {details.seasons.length > 0 && <p className="credit"><b>Seasons</b> {details.seasons.length} available</p>}{details.dubs.length > 0 && <p className="credit"><b>Audio options</b> {details.dubs.map((dub) => dub.label).join(", ")}</p>}
          <p className="provider-note">Details from {details.id.provider}. In-app playback is a later milestone.</p>
        </div>
      </section></div>}
    </div>
  );
}

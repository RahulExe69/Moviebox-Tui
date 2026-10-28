# Android app foundation plan

## Audit result

The current `main` branch is a Rust terminal application using Ratatui/Crossterm. Android support is through Termux and external-player hand-off; the repository does not yet contain a native Android app module or APK workflow.

However, **7xAnime already demonstrates a viable packaging pattern for this family of apps**. Its Capacitor frontend bundles a Python runtime and FastAPI backend in the APK, starts the API on `127.0.0.1:8000`, and keeps provider requests on-device without requiring us to host our own extraction server. See [RahulExe69/7xAnime](https://github.com/RahulExe69/7xAnime), `android-local-backend/README.md`, `android-local-backend/local_api.py`, and `.github/workflows/7xanime-build.yml`.

We should reuse the *architectural pattern*, not blindly copy its Python implementation. MovieBox-TUI's provider logic is Rust, so the best option is to expose the existing Rust provider service to the bundled UI through a small local API if Android compilation is feasible. That keeps us from rewriting every source adapter in Python.

## Implementation status on the feature branch

- The Android ARM64 feasibility workflow has passed for the existing Rust library at the initial implementation baseline. This established cross-compilation feasibility, not runtime correctness or APK readiness.
- `src/mobile_api.rs` defines a transport-independent `MobileApi` facade for provider discovery, typed search, details, MovieBox homepage data, playback-source discovery and selected-source resolution. It reuses the existing `MovieBoxService` and serializable models rather than duplicating provider logic.
- The facade validates blank query/ID/tab inputs, normalizes provider IDs through the existing parser, clamps requested pages and reports whether the optional 4KHDHub client initialized.
- `src/mobile_server.rs` exposes the facade through an authenticated HTTP API bound to loopback only, with a JSON body limit, bounded concurrent connections and a small route allowlist.
- `src/android_jni.rs` and a native Capacitor plugin connect Android to the Rust server. The bridge returns a per-process token and ephemeral loopback port without logging the token.
- `mobile/` contains the React + TypeScript search/details shell plus source listing, subtitle selection, and a play action that requests a resolved source from the local API.
- The native Capacitor bridge now opens a dedicated AndroidX Media3/ExoPlayer activity with provider request headers and supported external subtitle formats; the packaging script adds Media3 dependencies and registers the activity in the generated Android manifest.
- The ARM64 debug APK workflow has produced an artifact, and the user has confirmed their APK install/smoke test passed. The current playback implementation still needs a fresh APK build and a real stream playback test on a physical Android device; provider-by-provider success is not yet verified.

## What can be reused from 7xAnime

- Capacitor + bundled web frontend to produce a familiar Android APK build pipeline.
- An on-device API bound only to `127.0.0.1`, with a random per-process token or equivalent request guard.
- Build-time dependency/provider pruning so the APK includes only selected integrations.
- Guest/local-first mode for features that do not need accounts or shared sync.
- A provider allowlist and explicit real-device validation rather than assuming imported providers work.
- Existing 7x brand/release workflow conventions, adjusted for this repository.

Do **not** assume the 7xAnime Python/Chaquopy backend can run the Rust provider engine directly. It cannot without an additional interface or port. Also do not expose a local API on `0.0.0.0`; it should listen on loopback only.

## Current Rust architecture

- `src/providers/` defines the shared `Provider` trait and common catalog, details, episode, release, playback-source, and subtitle models.
- `src/service.rs` composes MovieBox, 4KHDHub, BDIX, Dramachi and Stremio add-on clients. It is a promising boundary for a UI-independent API, but it is currently shaped around the terminal app and should be audited before reuse.
- `src/player.rs` starts external players or Android intents. A native app should use its own in-app playback layer rather than treating this terminal playback orchestration as the Android player.
- `src/tui/` contains the Ratatui interface and should remain a separate frontend.
- `src/config.rs` and other modules use `moviebox-tui` paths and `MOVIEBOX_*` environment variables; rename and migration should be deliberate.
- `Cargo.toml` already declares a Rust library target as well as the terminal binary, which gives us a starting point for a bridge, but does not prove that the full dependency graph compiles for Android.
- Existing release workflows build desktop binaries; they do not package a native Android APK.

## Recommended architecture

### Preferred route: local Rust API + Capacitor UI

1. **Prove the Rust core on Android ARM64.** Set up the Android NDK/Cargo toolchain and test the provider/service dependencies without the TUI or desktop-only process code.
2. **Define a narrow, UI-independent provider facade.** Expose source capabilities, search, details, seasons/episodes, stream candidates, subtitles and diagnostics as typed serializable responses. Isolate Ratatui, terminal I/O, desktop player launching and process management.
3. **Run a local Rust API on loopback.** If the Rust dependency graph can be built and packaged reliably, provide a small HTTP API bundled in the APK and bind it only to `127.0.0.1`. Start/stop it from a native Capacitor bridge and guard requests with a random per-process token, following the security pattern used in 7xAnime.
4. **Bundle the frontend with Capacitor.** Use the 7xAnime-style web UI packaging flow, but create a 7xCinema-specific interface and API contract.
5. **Use in-app playback.** AndroidX Media3/ExoPlayer is now wired to play resolved direct streams with request headers and supported external subtitles. Next verify real streams, seek/resume, redirects and provider-specific limitations on device; embeds and unsupported stream types need explicit fallback handling.
6. **Keep providers modular.** Use an explicit allowlist, timeouts, per-provider diagnostics, normalized result types and failure isolation. Do not assume every upstream provider is stable or suitable for distribution.
7. **Preserve terminal functionality.** Keep the existing TUI usable; do not make Android-specific assumptions global to the Rust application.

### Fallback route: native Rust bridge

If a local HTTP server adds too much APK overhead or process complexity, evaluate a JNI/UniFFI bridge exposing the same narrow facade to a native or Capacitor frontend. Choose only after a minimal Android ARM64 build and a real-device proof of concept.

### Last resort: port selected providers

If key Rust dependencies are incompatible with Android, port only selected, reviewed provider adapters into the local Python backend pattern used by 7xAnime. Do not duplicate the entire provider catalogue by default; that would create two implementations to maintain and risks losing the existing Rust engine's behavior.

## Implementation milestones

### M1 — feasibility proof
- Audit `Cargo.toml`, terminal-only imports, networking, TLS/DNS, filesystem use and provider service dependencies.
- Add an Android ARM64 compile experiment on a feature branch.
- Confirm which providers can initialize and perform a search from the Android target.
- Record APK size and runtime constraints before choosing the final bridge.

### M2 — provider-core/API boundary
- Define stable JSON types and routes for source list, search, details, episodes, stream candidates, subtitles and diagnostics.
- Ensure one failing provider does not fail an aggregate search.
- Add timeouts, caching, cancellation and redacted error reporting.
- Keep local API loopback-only and authenticated with a per-process token.

### M3 — Android shell and playback
- Add Capacitor/Gradle packaging and a native bridge to start the local Rust service.
- Build a 7xCinema UI with browse, multi-source search, details, episodes, favourites and watch progress.
- Add a player that handles direct streams and embeds appropriately; verify actual playback on a physical Android device.

### M4 — branding and release pipeline
- Apply approved 7x brand assets, application ID, app icon, theme and versioning.
- Add Android debug APK artifacts and release checks without removing desktop CI.
- Preserve upstream licence notices and provide a third-party provider attribution inventory.

## Acceptance criteria for the first native release

- [ ] Produces an installable Android APK, not merely a Termux executable.
- [ ] Runs provider operations locally in the app process/device; no mandatory 7xCinema-owned scraping server for core browse/search.
- [ ] Searches at least two independently configured providers and isolates provider errors.
- [ ] Opens details and episodes where supported.
- [ ] Plays supported streams in-app with required request headers and subtitles handled.
- [ ] Persists favourites and watch progress locally.
- [ ] Binds any local API to loopback only and protects requests.
- [ ] Documents provider limitations and does not claim all sources are guaranteed to work.
- [ ] Preserves applicable upstream licences and only distributes authorised integrations.

## Scope and safety note

A provider integration is a connector to a third-party service, not permission to redistribute its content. Only configure and distribute provider integrations and stream access that are authorised for the intended use. Community providers can be unreliable, region-dependent, or unsuitable for public distribution. Test the actual playback path before treating a provider as supported.

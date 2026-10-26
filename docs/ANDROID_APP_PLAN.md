# Android app foundation plan

## Audit result

The current `main` branch is an unmodified source fork of [MovieBox-TUI](https://github.com/mesamirh/MovieBox-Tui). It is a Rust terminal application using Ratatui/Crossterm. Its Android support is **Termux**, where it launches an external player using Android intents; this repository does not currently contain a native Android app module, a Gradle project, an APK packaging workflow, or an Android release target.

The source is still a useful foundation because provider logic and data models live outside most of the terminal UI:

- `src/providers/` defines a shared `Provider` trait and common catalog, details, episode, release, playback-source, and subtitle models.
- `src/service.rs` composes the provider clients and exposes typed search/details and related provider operations.
- `src/player.rs` is desktop/Termux-specific playback orchestration. It starts external executables or Android intents; a native app should not use this as its in-app playback layer.
- `src/tui/` contains the Ratatui UI. It should remain the terminal frontend, not become the Android screen implementation.
- `src/config.rs` and multiple modules use `moviebox-tui` paths and `MOVIEBOX_*` variables; package/app renaming needs a deliberate compatibility plan.
- `.github/workflows/release.yml` builds desktop targets (Linux, macOS, Windows); it does not package an Android APK.

## Recommended architecture

Keep the existing terminal client functioning while adding a separate Android application. Do not start by globally renaming package identifiers or replacing provider code.

1. **Protect the baseline.** Keep this work on feature branches and retain upstream attribution and the MIT/Apache licence files.
2. **Extract/define a UI-independent core boundary.** Make provider search, catalog/details mapping, stream resolution, subtitle metadata, and settings callable without initializing Ratatui or relying on terminal process management. Keep provider clients behind a stable facade. Prefer a small core crate/module rather than duplicating scrapers in Kotlin.
3. **Prove Android compilation before designing the bridge.** Add an Android ARM64 compile check for the existing core first. Audit transitive native dependencies and filesystem/network assumptions. Do not assume that a Termux-compatible binary can be packaged as an APK.
4. **Add a native Android app module.** Use Kotlin with Jetpack Compose for the UI. Evaluate a narrow Rust bridge (for example UniFFI/JNI) only after a minimal Android target build succeeds; expose safe, typed operations instead of leaking internal provider structs.
5. **Use an in-app Android player.** Integrate AndroidX Media3/ExoPlayer for supported streams, with explicit handling for request headers, cookies, subtitle tracks, seek/resume, errors, and download permissions. Keep the old external-player flow for the terminal app.
6. **Separate app identity from upstream compatibility.** Set a stable application ID such as `com.sevenx.cinema` only once the Android module exists. Migrate local data/config deliberately, keep environment variable aliases for existing terminal users, and update updater/release endpoints, installer scripts, docs, and binary names together.
7. **Ship only after tests.** Build a signed/unsigned debug APK for ARM64, install it on an emulator or device, and verify search across multiple providers, provider failures, detail/episode navigation, playback, subtitles, history/resume, and offline/error states. Add CI for Android without removing the existing desktop test matrix.

## First implementation milestones

### M1 — provider-core boundary
- Map the smallest public API needed by a UI: source list/capabilities, search, details, releases/streams, and subtitles.
- Identify terminal-only imports and move them out of the core boundary.
- Add tests against the public facade and ensure the existing TUI builds and behaves as before.

### M2 — Android build proof
- Build only the core for Android ARM64 first.
- Record required NDK/toolchain settings and any unsupported dependencies.
- Confirm request headers and playback metadata can be passed safely to Kotlin.

### M3 — native Android shell
- Create the Gradle app module, Compose navigation, permission/error states, and a basic provider/search screen.
- Integrate the provider bridge before implementing visual polish.
- Add Media3 playback and subtitle selection.

### M4 — 7xCinema branding and releases
- Apply the approved 7x brand assets, app name, package ID, icons, theme and versioning.
- Add APK build artifacts and release checks; preserve the existing licence notices and source attribution.
- Publish only after real-device smoke tests pass.

## Acceptance criteria for the first native release

- [ ] Builds an installable Android APK; not just a Termux binary.
- [ ] Searches at least two independently configured providers and isolates provider errors.
- [ ] Opens details and episode lists where the provider supports them.
- [ ] Plays supported streams in-app with request headers and subtitles correctly handled.
- [ ] Persists favourites and watch progress locally.
- [ ] Does not claim that every provider or stream is guaranteed to work.
- [ ] Preserves applicable upstream licences and identifies lawful, authorised source use as the user's responsibility.

## Scope and safety note

A provider integration is a connector to a third-party service, not permission to redistribute its content. Only configure and distribute provider integrations and stream access that are authorised for the intended use. This plan does not assume that every community provider is reliable, lawful in every region, or suitable for inclusion in a public release.

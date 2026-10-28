<div align="center">

# 7xCinema

**A multi-source media client built on the MovieBox-TUI Rust project**

Explore a modular provider engine for movies, series, anime, and live TV—with a mobile-first Android experience planned as the next major milestone.

</div>

> [!IMPORTANT]
> **Development status:** 7xCinema is currently a development fork of [MovieBox-TUI](https://github.com/mesamirh/MovieBox-Tui). The inherited application is a terminal UI, not a native Android app. Android use currently means running it in **Termux** and opening streams in an external media player. A native Android APK has **not** been built or released yet.

## What the current codebase provides

The upstream Rust code currently includes:

- **Multiple provider integrations:** MovieBox, 4KHDHub, Dramachi, CircleFTP and DhakaFlix BDIX sources, plus community Stremio HTTP add-ons.
- **Movies and series:** provider search, details, seasons/episodes, and stream-resolution selection where supported.
- **Playback hand-off:** launches external players such as mpv or VLC; Android Termux can hand streams to installed Android players.
- **Subtitles:** provider subtitle discovery and subtitle handling where supported.
- **Downloads:** batch/season downloads with pause/resume support in the inherited client.
- **Local library:** favourites, watch history, continue-watching state, and local configuration.
- **Live TV:** M3U playlists, channel categories, and EPG support.

Provider availability and features vary. Some sources may be unavailable depending on the network or may change without notice.

## Android app roadmap

The goal is to evolve 7xCinema into an Android-first app without losing its multi-source foundation. One promising architecture is the on-device backend pattern already used by [7xAnime](https://github.com/RahulExe69/7xAnime): bundle the frontend and backend in the APK, start a loopback-only API on the phone, and contact providers directly without requiring a 7xCinema-owned scraping server. Because MovieBox-TUI's provider engine is Rust rather than Python, we must first prove Android ARM64 compatibility and choose a suitable Rust-to-UI bridge. The native UI and in-app playback are planned work—not existing features.

See [the Android app foundation plan](docs/ANDROID_APP_PLAN.md) for the architecture audit and proposed implementation sequence.

## Build the current terminal version

This is for developers who want to run the inherited Rust terminal application today.

```bash
git clone https://github.com/RahulExe69/7xCinema.git
cd 7xCinema
cargo build --release --locked
```

The current executable is still named `moviebox-tui` while the internal packaging and identifiers are being audited. Run it with:

```bash
./target/release/moviebox-tui
```

### Android (Termux)

The current Android route requires Termux and an installed external player. It is **not** an installable 7xCinema APK.

```bash
pkg update
pkg install rust git
git clone https://github.com/RahulExe69/7xCinema.git
cd 7xCinema
cargo build --release --locked
```

If your Termux environment needs additional Rust build tools, install the packages indicated by any compiler error. To play media, install a compatible Android player and use the terminal app's player hand-off.

## Upstream, attribution, and licence

7xCinema is derived from [MovieBox-TUI by mesamirh](https://github.com/mesamirh/MovieBox-Tui). The inherited source is dual-licensed under **MIT OR Apache-2.0**; the original licence files are retained in this repository. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE). Keep applicable copyright notices and attribution when distributing derived builds.

7xCinema is an independent project and is not affiliated with upstream providers or any streaming service. This application does not host media. Configure and use only sources and streams you are authorised to access, and follow local law and each source's terms.

## Privacy

The inherited terminal application documents local storage of configuration, history, and favourites and no telemetry. Any new mobile app components must be reviewed separately before making privacy claims about a released 7xCinema app.

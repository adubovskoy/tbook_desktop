# TReader — Desktop

A cross-platform (Linux/Windows/macOS) build of the TReader language-learning ebook
reader. You read the original text; click any word to see the **full sentence
translation** with the aligned word(s) **highlighted**, plus **IPA** pronunciation for
English source text. Books are the offline **`.tbook`** format (same files as the Android
app and the `../converter`).

Built with **Tauri v2** — a small Rust core (`.tbook` parsing, library, IPA dictionary,
search) plus a **Svelte 5 + TypeScript** WebView UI. The OS WebView does the heavy text
work (layout, word hit-testing, italic/bold, search highlighting, and CSS-column
pagination), which keeps the binary tiny (~6 MB; ~4.8 MB `.deb`).

## Feature parity with the Android app

- **Library**: covers, import a `.tbook` (native file dialog), delete, bundled sample on
  first run, show-covers toggle.
- **Reader**: tappable words → translation + IPA bottom sheet with aligned-word
  highlighting; paged (CSS columns) **and** scrolled modes; chapter next/prev + chapter
  list; per-book reading position & weighted progress; in-book search with highlighting;
  click background to toggle chrome; keyboard paging (←/→, PageUp/Down, Space).
- **Settings**: font family (Serif/Sans/Mono), font size, theme (Light/Dark/System),
  reading mode, margins. Gloss language is chosen via chips in the reader.

The `.tbook` format and all reader algorithms are ported from the Android sources
(`../android/.../data/model/Models.kt`, `ParagraphText.kt`, `BookSearch.kt`,
`BookProgress.kt`, `PronunciationDictionary.kt`, `SettingsRepository.kt`). Pagination is
handled by the WebView (CSS multi-column) instead of Android's `ChapterPaginator`.

## Prerequisites

- **Rust** (stable) and **Node.js 18+**.
- **Linux**: a WebKitGTK 4.1 dev stack. On Arch: `webkit2gtk-4.1 base-devel`
  (Debian/Ubuntu: `libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev`).
- **Windows**: WebView2 runtime (preinstalled on Windows 10/11). **macOS**: Xcode CLT.

## Develop & build

```bash
npm install
npm run tauri dev      # run with hot-reload
npm run check          # type-check the frontend (svelte-check)
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit + sample integration tests
npm run tauri build    # release bundle(s) → src-tauri/target/release/bundle/
npm run tauri build -- --bundles deb   # just the .deb
```

Bundled assets (`en_ipa.tsv`, `sample.tbook`) live in `src-tauri/resources/` and are copied
from `../android/app/src/main/assets/`.

## Release builds (CI)

`.github/workflows/release.yml` builds installers for all three OSes and attaches
them to a **draft** GitHub release:

- **Windows**: `.msi` (WiX) + `-setup.exe` (NSIS)
- **macOS**: `.dmg` for Apple Silicon (`aarch64`) and Intel (`x64`)
- **Linux**: `.deb`, `.rpm`, `.AppImage` (built on Ubuntu 22.04 for wide glibc compat)

To cut a release:

1. Bump `version` in `src-tauri/tauri.conf.json` (and `package.json` to keep them in sync).
2. Tag and push — the tag **must** match the app version:
   ```bash
   git tag v0.1.0 && git push origin v0.1.0
   ```
3. Wait for the four matrix jobs, then review and **publish the draft release**.

The workflow can also be started manually from the Actions tab (workflow_dispatch);
it then creates the `v<version>` tag/draft itself. macOS bundles are unsigned —
users open them via right-click → Open (or `xattr -d com.apple.quarantine`); add
Apple signing/notarization secrets to the workflow later if needed.

## Layout

```
src/                  Svelte frontend
  lib/                pure logic: types, render, align, search, progress, settings, app state
  screens/            Library, Reader, Settings
  components/         Paragraph, TranslationSheet, ChapterList, SearchPanel
src-tauri/src/        Rust: tbook (ZIP+JSON), library, ipa, search, lib (commands)
src-tauri/resources/  en_ipa.tsv, sample.tbook
```

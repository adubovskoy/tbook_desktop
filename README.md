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

- **Library**: covers, import a `.tbook` (native file dialog **or** drag-and-drop),
  delete, bundled sample on first run, show-covers toggle, most-recently-opened first.
- **Reader**: clickable words → translation + IPA popup with aligned-word highlighting
  and read-aloud; **bilingual mode**; **offline dictionary articles**; footnotes,
  figures, tables; the cover as the book's first page; paged (CSS columns) **and**
  scrolled modes; chapter next/prev + chapter list + page slider; per-book reading
  position & weighted progress; in-book search with highlighting and match stepping
  (`n`/`p`); click background to toggle chrome; keyboard paging (←/→, PageUp/Down,
  Space) or click-to-turn.
- **Bilingual mode** (the `/` button in the reader's top bar): the gloss is *baked into
  the paragraph render*, not overlaid — `buildParagraphRender(…, gloss)` appends each
  sentence's translation to the paragraph text (recorded in `ParagraphRender.glossRuns`,
  drawn paler) and turns every align chunk into a tappable gloss span. A click then
  highlights the pair on both sides (`pairRanges`) and opens the popup pinned to the
  dictionary article; a click on the interleaved gloss itself only highlights, its index
  addressing an align chunk rather than a word a dictionary could look up.
- **Read-aloud** (`src/lib/tts.ts` + `src-tauri/src/tts.rs`): `window.speechSynthesis`
  where the WebView has it (WebView2 on Windows, WKWebView on macOS), otherwise a Rust
  command driving the system speech CLI (`spd-say`, `espeak-ng`, `say`). WebKitGTK
  builds speech synthesis behind a compile-time flag most distributions leave off, hence
  the fallback. The speaker button is hidden when neither can speak the source language.
  A click reads the word, a hold or right-click the sentence.
- **Offline dictionaries** (`src-tauri/src/dict/`, `src/screens/Dictionaries.svelte`):
  per-language-pair `.tdict` SQLite files (spec: `../android/docs/tdict-format.md`, built
  by `../dictionaries/`) downloaded from dictionary.tbook.dev into
  `<app data>/dictionaries/`. Streaming download → sha256 → gunzip → validate → atomic
  commit, with progress/cancel over `dict-progress` events; the list is filtered to the
  language pairs of books actually in the library. This and the catalog fetch are the
  app's only network use.
- **Pronunciation**: two bundled IPA dictionaries — `resources/en_ipa_us.tsv` (General
  American) and `en_ipa_gb.tsv` (Received Pronunciation) — picked by the Accent setting,
  loaded lazily and one accent at a time. The same setting pins the read-aloud voice for
  books tagged plain `en`. Transcriptions are set in a bundled Charis subset.
- **Settings**: font family/size, popup text sizes, theme, accent, reading mode, text
  alignment, click action, default translation language (+ hide the popup's language
  picker), margins, offline dictionaries, "How to use", credits.

The `.tbook` format and all reader algorithms are ported from the Android sources
(`../android/.../data/model/Models.kt`, `ParagraphText.kt`, `BookSearch.kt`,
`BookProgress.kt`, `PronunciationDictionary.kt`, `EnglishSpelling.kt`,
`SettingsRepository.kt`, `data/dict/*`, `data/tts/Speaker.kt`). Pagination is handled by
the WebView (CSS multi-column) instead of Android's `ChapterPaginator`.

**Deliberately not ported**: everything e-ink (`ui/eink/`), the reading status bar's
clock/battery readout (the desktop has both in the system bar), chapter switching by
pulling past a scroll edge (`EdgePull.kt` — desktop has explicit chapter buttons and
keys), and the nine locales of the help page (the desktop help is English only; the
phone's copy lives in `res/values-*/strings.xml`). `.tbook` **file associations** are
also not registered yet — books are imported by dialog or drag-and-drop.

## Prerequisites

- **Rust** (stable) and **Node.js 20+** (CI builds on Node 24).
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

Building the **AppImage** locally on a rolling distro (Arch & co.) needs
`NO_STRIP=1 npm run tauri build -- --bundles appimage`: linuxdeploy ships an old
`strip` that chokes on `.relr.dyn` sections in current system libraries. Then run
`scripts/patch-appimage.sh` on the result (see below) — the raw AppImage starts
with a white window.

Bundled assets (`en_ipa_us.tsv`, `en_ipa_gb.tsv`, `sample.tbook`) live in
`src-tauri/resources/` and are copied from `../android/app/src/main/assets/`. The IPA
font (`src/assets/ipa.ttf`, from `../android/app/src/main/res/font/`) is bundled by Vite
instead, so the WebView can load it as an ordinary asset.

The Rust side pulls in `rusqlite` (bundled SQLite, for `.tdict` lookups), `ureq` +
`sha2` + `flate2` (dictionary downloads). `rusqlite` is pinned to 0.37: 0.40's
`libsqlite3-sys` build script needs an unstable Rust feature.

Network tests are opt-in — `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored`
runs the one that downloads the smallest real dictionary from dictionary.tbook.dev and
looks a word up in it.

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
it then creates the `v<version>` tag/draft itself.

**AppImage white window**: linuxdeploy bundles the build machine's
`libwayland-client/-cursor/-egl/-server`, while `libEGL`/`libGL`/`libdrm` are
deliberately taken from the host (they have to match its driver). The host's Mesa
then refuses to initialise against the older bundled libwayland, WebKit aborts
with `Could not create default EGL display: EGL_BAD_PARAMETER` and the window
stays white — on any current Wayland desktop, and no
`WEBKIT_DISABLE_DMABUF_RENDERER`/`WEBKIT_DISABLE_COMPOSITING_MODE` helps. The
release workflow therefore runs `scripts/patch-appimage.sh` after the build: it
strips those libraries (GTK then links the host's, which by construction matches
the host's Mesa), repacks the AppImage and re-uploads it. Useful standalone too:

```bash
scripts/patch-appimage.sh --check some.AppImage   # is libwayland bundled?
scripts/patch-appimage.sh some.AppImage           # strip + repack in place
```

**macOS Gatekeeper**: bundles are ad-hoc signed (`signingIdentity: "-"` in
`tauri.conf.json`) but not notarized. The signature itself is valid, there is just
no notarization ticket, so the first launch of a downloaded build is blocked with
*"Apple could not verify «TReader» is free of malware…"* (older systems report
*"TReader is damaged"* instead). Since macOS Sequoia right-click → Open no longer
bypasses this. Users must either click **Open Anyway** in System Settings →
Privacy & Security after the blocked launch, or clear the quarantine flag once:

```bash
xattr -dr com.apple.quarantine /Applications/TReader.app
```

The proper fix is Apple Developer signing + notarization: add the
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` secrets as `env` on the
tauri-action step (and drop the ad-hoc `signingIdentity`); tauri-action then
signs and notarizes automatically.

## Layout

```
src/                  Svelte frontend
  lib/                pure logic: types, render, align, search, progress, settings,
                      dict, tts, app state
  screens/            Library, Reader, Settings, Dictionaries, Help
  components/         Paragraph, TranslationSheet, DictionaryArticle, NoteSheet,
                      ChapterList, SearchPanel
  assets/             ipa.ttf (Charis subset, for transcriptions)
src-tauri/src/        Rust: tbook (ZIP+JSON), library, ipa, search, tts, lib (commands)
  dict/               .tdict: models, normalize, store (SQLite), manifest, download
src-tauri/resources/  en_ipa_us.tsv, en_ipa_gb.tsv, sample.tbook
```

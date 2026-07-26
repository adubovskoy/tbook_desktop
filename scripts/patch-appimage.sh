#!/usr/bin/env bash
# Strip the bundled libwayland-* out of a Tauri AppImage and repack it.
#
# Why: linuxdeploy bundles libwayland-client/-cursor/-egl/-server from the build
# machine (Ubuntu 22.04 on CI, wayland 1.20). Those libraries must come from the
# host instead: libEGL/libGL/libdrm are deliberately *not* bundled (they are on
# the AppImage excludelist, since they have to match the host driver), and the
# host Mesa libEGL then refuses to initialise against the older bundled
# libwayland-client. WebKit's GPU init dies with
#
#     Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
#
# and the app window stays white — reproducible on any current Wayland desktop
# (seen with Mesa 26 / KDE Wayland, and not fixable with
# WEBKIT_DISABLE_DMABUF_RENDERER / WEBKIT_DISABLE_COMPOSITING_MODE).
#
# Removing them makes GTK/WebKit link the host's libwayland, which by definition
# matches the host's Mesa. Every desktop distro ships libwayland (GTK3 itself
# depends on it), so nothing is lost.
#
# Usage:
#   scripts/patch-appimage.sh [App.AppImage]   patch in place (default: the
#                                              AppImage in the bundle dir)
#   scripts/patch-appimage.sh --check App.AppImage
#                                              exit 1 if libwayland is bundled
set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/src-tauri/target/release/bundle/appimage"

check_only=0
if [ "${1:-}" = "--check" ]; then
  check_only=1
  shift
fi

appimage="${1:-}"
if [ -z "$appimage" ]; then
  appimage="$(find "$BUNDLE_DIR" -maxdepth 1 -name '*.AppImage' -print -quit 2>/dev/null || true)"
fi
if [ -z "$appimage" ] || [ ! -f "$appimage" ]; then
  echo "error: no AppImage found (looked in $BUNDLE_DIR)" >&2
  exit 1
fi
appimage="$(readlink -f "$appimage")"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The AppImage runtime only extracts into the current directory.
(cd "$work" && "$appimage" --appimage-extract >/dev/null)
appdir="$work/squashfs-root"

bundled=$(find "$appdir/usr/lib" -maxdepth 1 -name 'libwayland-*' -printf '%f\n' 2>/dev/null | sort)

if [ "$check_only" = 1 ]; then
  if [ -n "$bundled" ]; then
    echo "FAIL: $(basename "$appimage") still bundles libwayland:" >&2
    echo "$bundled" | sed 's/^/  /' >&2
    exit 1
  fi
  echo "OK: $(basename "$appimage") bundles no libwayland"
  exit 0
fi

if [ -z "$bundled" ]; then
  echo "$(basename "$appimage"): no bundled libwayland, nothing to do"
  exit 0
fi

echo "$(basename "$appimage"): removing bundled"
echo "$bundled" | sed 's/^/  /'
find "$appdir/usr/lib" -maxdepth 1 -name 'libwayland-*' -delete

# Repack with the appimage plugin Tauri already downloaded during the build;
# fall back to appimagetool for a standalone run.
repack=""
for candidate in "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage" \
                 "$BUNDLE_DIR/linuxdeploy-plugin-appimage.AppImage"; do
  [ -x "$candidate" ] && repack="$candidate" && break
done

out="$work/repacked.AppImage"
if [ -n "$repack" ]; then
  # OUTPUT is what current versions read, LDAI_OUTPUT what they ask for.
  ARCH=x86_64 OUTPUT="$out" LDAI_OUTPUT="$out" \
    "$repack" --appimage-extract-and-run --appdir "$appdir" >/dev/null
else
  repack="$work/appimagetool.AppImage"
  echo "downloading appimagetool…"
  curl -fsSL -o "$repack" \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$repack"
  ARCH=x86_64 "$repack" --appimage-extract-and-run "$appdir" "$out" >/dev/null
fi
[ -f "$out" ] || { echo "error: repack produced no AppImage" >&2; exit 1; }

mv "$out" "$appimage"
chmod +x "$appimage"
echo "$(basename "$appimage"): repacked ($(du -h "$appimage" | cut -f1))"

# Verify the result rather than trusting the repack.
probe="$work/probe"
mkdir -p "$probe"
(cd "$probe" && "$appimage" --appimage-extract 'usr/lib/libwayland-*' >/dev/null 2>&1 || true)
if find "$probe" -name 'libwayland-*' | grep -q .; then
  echo "error: libwayland still present after repack" >&2
  exit 1
fi
echo "$(basename "$appimage"): verified — libwayland comes from the host"

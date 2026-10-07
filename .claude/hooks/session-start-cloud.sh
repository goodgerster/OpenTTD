#!/bin/bash
# SessionStart hook for Claude Code cloud sessions (run by session-start.sh).
#
# Installs the libraries used by a headless (dedicated) build, the build
# accelerators (ccache, mold, Ninja), the pinned Rust toolchain and the OpenGFX
# baseset needed by the regression tests, and configures build/. When a new
# session starts, it then builds build/ in the background, which also fills
# ccache; build/warm-build.done appears when that build has finished.
#
# Everything here is idempotent; a failed step prints a warning and the
# session continues.

set -uo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
	exit 0
fi

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
BUILD_DIR="$PROJECT_DIR/build"
BASESET_SRC=/usr/share/games/openttd/baseset/opengfx
BASESET_DST="$HOME/.local/share/openttd/baseset"

# The optional libraries CMake looks for in a dedicated build (see CMakeLists.txt),
# matching the CI apt list minus the GUI-only ones; build accelerators; and
# Ubuntu's OpenGFX package (cdn.openttd.org, which CI uses, may be blocked by the
# network policy).
PACKAGES=(
	liblzma-dev
	zlib1g-dev
	libpng-dev
	libzstd-dev
	liblzo2-dev
	libcurl4-openssl-dev
	ccache
	mold
	ninja-build
	openttd-opengfx
)

status=()

missing=()
for pkg in "${PACKAGES[@]}"; do
	dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null | grep -q 'install ok installed' || missing+=("$pkg")
done

if [ ${#missing[@]} -gt 0 ]; then
	SUDO=""
	[ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null && SUDO="sudo"
	if $SUDO env DEBIAN_FRONTEND=noninteractive apt-get update -qq >/dev/null 2>&1 &&
			$SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends "${missing[@]}" >/dev/null 2>&1; then
		status+=("installed: ${missing[*]}")
	else
		status+=("WARNING: apt-get failed to install: ${missing[*]}")
	fi
fi

if [ -d "$BASESET_SRC" ]; then
	mkdir -p "$BASESET_DST"
	ln -sfn "$BASESET_SRC" "$BASESET_DST/opengfx"
	status+=("baseset: OpenGFX linked into $BASESET_DST")
else
	status+=("WARNING: no OpenGFX baseset; regression_* tests will fail")
fi

# The Rust toolchain pinned in rust-toolchain.toml; CMake configure fails without it.
RUST_CHANNEL="$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$PROJECT_DIR/rust-toolchain.toml")"
if command -v rustup >/dev/null; then
	if rustup toolchain install "$RUST_CHANNEL" --profile minimal --component rustfmt,clippy >/dev/null 2>&1 &&
			(cd "$PROJECT_DIR/rust" && cargo fetch --locked >/dev/null 2>&1); then
		status+=("Rust $RUST_CHANNEL installed, crates fetched")
	else
		status+=("WARNING: installing Rust $RUST_CHANNEL or fetching crates failed")
	fi
else
	status+=("WARNING: rustup not found; install Rust $RUST_CHANNEL")
fi

# Configure only if build/ has not been configured yet, so an existing configuration is left alone.
if [ ! -f "$BUILD_DIR/CMakeCache.txt" ]; then
	mkdir -p "$BUILD_DIR"
	if cmake -S "$PROJECT_DIR" -B "$BUILD_DIR" -G Ninja -DOPTION_DEDICATED=ON \
			-DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
			-DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=mold >"$BUILD_DIR/configure.log" 2>&1; then
		status+=("build/ configured (Ninja, dedicated, Debug, ccache, mold)")
	else
		status+=("WARNING: CMake configure failed, see build/configure.log")
	fi
else
	status+=("build/ already configured, left unchanged")
fi

if command -v ccache >/dev/null; then
	status+=("ccache: $(ccache -sv 2>/dev/null | sed -n 's/^ *Files: *\([0-9]*\).*/\1/p' | head -1) files cached in $(ccache -k cache_dir 2>/dev/null)")
fi

# Build in the background for new sessions only: a resumed session may be in the middle of its own build.
warm_build=false
if [ "${HOOK_SOURCE:-startup}" = "startup" ] && [ -f "$BUILD_DIR/CMakeCache.txt" ]; then
	rm -f "$BUILD_DIR/warm-build.done"
	setsid nohup nice -n 10 bash -c 'cmake --build "$1" --target all openttd_test >"$1/warm-build.log" 2>&1; echo "exit status $?" >"$1/warm-build.done"' \
		_ "$BUILD_DIR" </dev/null >/dev/null 2>&1 &
	warm_build=true
	status+=("background build of build/ started (log: build/warm-build.log)")
fi

echo "OpenTTD session setup:"
printf '  - %s\n' "${status[@]}"
if $warm_build; then
	echo "  Wait until build/warm-build.done exists (it holds the build's exit status) before building or testing in"
	echo "  build/; never run a second build in build/ at the same time. With a warm ccache this takes about a minute."
fi
echo "  Build: cmake --build build --target all openttd_test"
echo "  Test:  ctest --test-dir build -j\$(nproc) --timeout 120"
exit 0

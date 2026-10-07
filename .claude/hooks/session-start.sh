#!/bin/bash
# SessionStart hook for Claude Code cloud sessions.
#
# Installs the libraries used by a headless (dedicated) build, installs the
# OpenGFX baseset needed by the regression tests, and configures build/ so a
# session can go straight to building and testing. It does not compile
# anything: a full build takes about 10 minutes on 4 cores.
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
# matching the CI apt list minus the GUI-only ones, plus Ubuntu's OpenGFX package.
# cdn.openttd.org, which CI downloads OpenGFX from, may be blocked by the network policy.
PACKAGES=(
	liblzma-dev
	zlib1g-dev
	libpng-dev
	libzstd-dev
	liblzo2-dev
	libcurl4-openssl-dev
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

# Configure only if build/ has not been configured yet, so an existing configuration is left alone.
if [ ! -f "$BUILD_DIR/CMakeCache.txt" ]; then
	mkdir -p "$BUILD_DIR"
	if cmake -S "$PROJECT_DIR" -B "$BUILD_DIR" -DOPTION_DEDICATED=ON >"$BUILD_DIR/configure.log" 2>&1; then
		status+=("build/ configured (dedicated, Debug)")
	else
		status+=("WARNING: CMake configure failed, see build/configure.log")
	fi
else
	status+=("build/ already configured, left unchanged")
fi

echo "OpenTTD session setup:"
printf '  - %s\n' "${status[@]}"
echo "  Build: cmake --build build -j\$(nproc) --target all openttd_test (about 10 min from scratch)"
echo "  Test:  ctest --test-dir build -j\$(nproc) --timeout 120"
exit 0

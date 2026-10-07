#!/bin/bash
# SessionStart hook for local sessions on macOS (run by session-start.sh).
#
# Loads the Nix dev shell from flake.nix (toolchain, Rust, libraries) into the
# environment of Claude's Bash commands, installs the OpenGFX baseset needed by
# the regression tests, and configures build/ (Ninja, ccache, GUI build).
#
# Needs Nix (e.g. from nix-darwin); flakes are enabled for these commands even
# if nix.conf does not enable them. The first run downloads the toolchain into the Nix store and writes
# flake.lock (commit it). Nothing is installed outside the Nix store and
# ~/Documents/OpenTTD/baseset. A failed step prints a warning and the session
# continues.
#
# OPENTTD_NIX_ARGS adds arguments to the nix commands (e.g. --override-input).

set -uo pipefail
# Note: this runs under macOS's /bin/bash 3.2, where expanding an empty array
# under set -u fails; use ${array[@]+"${array[@]}"}.

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$PROJECT_DIR/build"
ENV_SCRIPT="$BUILD_DIR/nix-dev-env.sh"
BASESET_DIR="$HOME/Documents/OpenTTD/baseset"
OPENGFX_URL="https://cdn.openttd.org/opengfx-releases/0.6.0/opengfx-0.6.0-all.zip"
NIX_ARGS=(--extra-experimental-features "nix-command flakes")
read -r -a EXTRA_NIX_ARGS <<< "${OPENTTD_NIX_ARGS:-}"
NIX_ARGS+=(${EXTRA_NIX_ARGS[@]+"${EXTRA_NIX_ARGS[@]}"})

# Sessions started from the desktop app may not have nix-darwin's paths in PATH.
for dir in /run/current-system/sw/bin /nix/var/nix/profiles/default/bin "$HOME/.nix-profile/bin"; do
	[ -d "$dir" ] && PATH="$PATH:$dir"
done

status=()
report() {
	echo "OpenTTD session setup:"
	printf '  - %s\n' "${status[@]}"
}

if ! command -v nix >/dev/null; then
	status+=("WARNING: nix not found, so the build environment was not set up (see flake.nix)")
	report
	exit 0
fi

mkdir -p "$BUILD_DIR"

# Build the dev shell (downloads it on first use) and turn its environment into
# export lines. The profile is a garbage-collector root for the dev shell.
if nix print-dev-env ${NIX_ARGS[@]+"${NIX_ARGS[@]}"} --json --profile "$BUILD_DIR/nix-dev-shell" "$PROJECT_DIR" \
		>"$BUILD_DIR/nix-dev-env.json" 2>"$BUILD_DIR/nix-dev-env.log" &&
		nix eval ${NIX_ARGS[@]+"${NIX_ARGS[@]}"} --raw --impure \
			--expr "import $HOOK_DIR/dev-env-exports.nix \"$BUILD_DIR/nix-dev-env.json\"" \
			>"$ENV_SCRIPT" 2>>"$BUILD_DIR/nix-dev-env.log"; then
	if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
		echo "source '$ENV_SCRIPT'" >>"$CLAUDE_ENV_FILE"
		status+=("Nix dev shell loaded for Bash commands (build/nix-dev-env.sh)")
	else
		status+=("Nix dev shell ready, but CLAUDE_ENV_FILE is not set: prefix commands with 'source build/nix-dev-env.sh &&'")
	fi
else
	status+=("WARNING: building the Nix dev shell failed, see build/nix-dev-env.log")
	report
	exit 0
fi

have_opengfx=false
for f in "$BASESET_DIR"/opengfx*.tar "$BASESET_DIR"/*/opengfx.obg; do
	[ -e "$f" ] && have_opengfx=true
done
if ! $have_opengfx; then
	mkdir -p "$BASESET_DIR"
	if curl -sSfL "$OPENGFX_URL" -o "$BASESET_DIR/opengfx-all.zip" &&
			unzip -qo "$BASESET_DIR/opengfx-all.zip" -d "$BASESET_DIR"; then
		status+=("baseset: OpenGFX installed in $BASESET_DIR")
	else
		status+=("WARNING: downloading OpenGFX failed; regression_* tests will fail")
	fi
	rm -f "$BASESET_DIR/opengfx-all.zip"
fi

# Configure only if build/ has not been configured yet, so an existing configuration is left alone.
if [ ! -f "$BUILD_DIR/CMakeCache.txt" ]; then
	if (source "$ENV_SCRIPT" && cmake -S "$PROJECT_DIR" -B "$BUILD_DIR" -G Ninja \
			-DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache) \
			>"$BUILD_DIR/configure.log" 2>&1; then
		status+=("build/ configured (Ninja, Debug, ccache)")
	else
		status+=("WARNING: CMake configure failed, see build/configure.log")
	fi
else
	status+=("build/ already configured, left unchanged")
fi

report
echo "  Build: cmake --build build --target all openttd_test"
echo "  Test:  ctest --test-dir build -j\$(sysctl -n hw.logicalcpu) --timeout 120"
exit 0

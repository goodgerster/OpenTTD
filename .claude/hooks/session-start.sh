#!/bin/bash
# SessionStart hook: prepares the build environment from the Nix flake
# (flake.nix), in Claude Code cloud sessions (Linux) and local sessions on
# macOS. Elsewhere it does nothing.
#
# - Cloud sessions: installs Nix if it is missing.
# - Loads the dev shell into the environment of Claude's Bash commands:
#   build/nix-dev-env.sh, sourced through CLAUDE_ENV_FILE.
# - Installs the OpenGFX baseset needed by the regression tests.
# - macOS: makes Apple's git work with the dev shell's environment.
# - Configures build/ (Ninja, ccache, lld) unless it is configured.
# - Cloud sessions, new session only: builds build/ in the background, which
#   also fills ccache; build/warm-build.done appears when it has finished.
#
# Idempotent; a failed step prints a warning and the session continues.
# OPENTTD_NIX_ARGS adds arguments to the nix commands (e.g. --override-input).
#
# This runs under macOS's /bin/bash 3.2, where expanding an empty array under
# set -u fails; use ${array[@]+"${array[@]}"}.

set -uo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
	PLATFORM=cloud
	BASESET_DIR="$HOME/.local/share/openttd/baseset"
elif [ "$(uname -s)" = "Darwin" ]; then
	PLATFORM=macos
	BASESET_DIR="$HOME/Documents/OpenTTD/baseset"
else
	exit 0
fi

# The session start reason (startup, resume, clear, ...) from the hook input.
input=""
[ -t 0 ] || input="$(cat)"
HOOK_SOURCE="$(printf '%s' "$input" | sed -n 's/.*"source"[[:space:]]*:[[:space:]]*"\([a-z]*\)".*/\1/p')"

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$PROJECT_DIR/build"
ENV_SCRIPT="$BUILD_DIR/nix-dev-env.sh"
OPENGFX_URL="https://cdn.openttd.org/opengfx-releases/0.6.0/opengfx-0.6.0-all.zip"

NIX_ARGS=(--extra-experimental-features "nix-command flakes")
read -r -a EXTRA_NIX_ARGS <<< "${OPENTTD_NIX_ARGS:-}"
NIX_ARGS+=(${EXTRA_NIX_ARGS[@]+"${EXTRA_NIX_ARGS[@]}"})

# Sessions started from the desktop app may not have Nix's paths in PATH.
for dir in /run/current-system/sw/bin /nix/var/nix/profiles/default/bin "$HOME/.nix-profile/bin"; do
	[ -d "$dir" ] && PATH="$PATH:$dir"
done

status=()
report() {
	echo "OpenTTD session setup:"
	printf '  - %s\n' ${status[@]+"${status[@]}"}
}

mkdir -p "$BUILD_DIR"

# Cloud sessions: install Nix (single-user, as root) if it is missing.
if [ "$PLATFORM" = cloud ] && ! command -v nix >/dev/null; then
	mkdir -p /etc/nix
	[ -f /etc/nix/nix.conf ] || echo "build-users-group =" >/etc/nix/nix.conf
	if curl -sSfL https://nixos.org/nix/install -o "$BUILD_DIR/nix-install.sh" &&
			sh "$BUILD_DIR/nix-install.sh" --no-daemon --yes --no-channel-add --no-modify-profile \
				>"$BUILD_DIR/nix-install.log" 2>&1; then
		PATH="$PATH:/nix/var/nix/profiles/default/bin"
		status+=("installed Nix")
	fi
fi
if ! command -v nix >/dev/null; then
	status+=("WARNING: nix not found, so the build environment was not set up (see flake.nix)")
	report
	exit 0
fi

# Build the dev shell (downloaded on first use) and turn its environment into
# export lines. The profile is a garbage-collector root for the dev shell.
if nix print-dev-env ${NIX_ARGS[@]+"${NIX_ARGS[@]}"} --json --profile "$BUILD_DIR/nix-dev-shell" "$PROJECT_DIR" \
		>"$BUILD_DIR/nix-dev-env.json" 2>"$BUILD_DIR/nix-dev-env.log" &&
		nix eval ${NIX_ARGS[@]+"${NIX_ARGS[@]}"} --raw --impure \
			--expr "import $HOOK_DIR/dev-env-exports.nix \"$BUILD_DIR/nix-dev-env.json\"" \
			>"$ENV_SCRIPT" 2>>"$BUILD_DIR/nix-dev-env.log"; then
	# macOS: the dev shell sets DEVELOPER_DIR to the SDK from Nix, where Apple's git (an xcrun
	# shim) finds no git. Put a wrapper in front of it that runs it without that SDK.
	if [ "$PLATFORM" = macos ] && [ "$(command -v git)" = /usr/bin/git ]; then
		mkdir -p "$BUILD_DIR/nix-dev-bin"
		printf '%s\n' '#!/bin/sh' 'exec env -u DEVELOPER_DIR -u SDKROOT /usr/bin/git "$@"' \
			>"$BUILD_DIR/nix-dev-bin/git"
		chmod +x "$BUILD_DIR/nix-dev-bin/git"
		echo "export PATH='$BUILD_DIR/nix-dev-bin':\"\$PATH\"" >>"$ENV_SCRIPT"
	fi
	if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
		# A resumed session runs this hook again with the same file.
		grep -qxF "source '$ENV_SCRIPT'" "$CLAUDE_ENV_FILE" 2>/dev/null ||
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
	elif [ "$PLATFORM" = cloud ] &&
			env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq openttd-opengfx >/dev/null 2>&1; then
		ln -sfn /usr/share/games/openttd/baseset/opengfx "$BASESET_DIR/opengfx"
		status+=("baseset: Ubuntu's OpenGFX linked into $BASESET_DIR")
	else
		status+=("WARNING: installing OpenGFX failed; regression_* tests will fail")
	fi
	rm -f "$BASESET_DIR/opengfx-all.zip"
fi

# Configure only if build/ has not been configured yet, so an existing configuration is left alone.
configured=false
if [ ! -f "$BUILD_DIR/CMakeCache.txt" ]; then
	if (source "$ENV_SCRIPT" && cmake -S "$PROJECT_DIR" -B "$BUILD_DIR" -G Ninja \
			-DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
			-DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=lld) >"$BUILD_DIR/configure.log" 2>&1; then
		configured=true
		status+=("build/ configured (Ninja, ccache, lld)")
	else
		# Remove the partial cache, so that the next session configures again.
		rm -f "$BUILD_DIR/CMakeCache.txt"
		status+=("WARNING: CMake configure failed, see build/configure.log")
	fi
else
	configured_cxx="$(sed -n 's/^CMAKE_CXX_COMPILER:[A-Z]*=//p' "$BUILD_DIR/CMakeCache.txt")"
	dev_shell_cxx="$(source "$ENV_SCRIPT" && command -v "$CXX")"
	if [ "$configured_cxx" = "$dev_shell_cxx" ]; then
		configured=true
		status+=("build/ already configured, left unchanged")
	else
		status+=("WARNING: build/ was configured with $configured_cxx, not the dev shell's compiler; delete build/ to reconfigure")
	fi
fi

ccache_files="$(source "$ENV_SCRIPT" && ccache -sv 2>/dev/null | sed -n 's/^ *Files: *\([0-9]*\).*/\1/p' | head -1)"
[ -n "$ccache_files" ] && status+=("ccache: $ccache_files files cached")

# Build in the background for new cloud sessions only: a resumed session may be in the middle of its own build.
warm_build=false
if [ "$PLATFORM" = cloud ] && [ "${HOOK_SOURCE:-startup}" = "startup" ] && $configured; then
	rm -f "$BUILD_DIR/warm-build.done"
	setsid nohup nice -n 10 bash -c \
		'source "$2"; cmake --build "$1" --target all openttd_test >"$1/warm-build.log" 2>&1; echo "exit status $?" >"$1/warm-build.done"' \
		_ "$BUILD_DIR" "$ENV_SCRIPT" </dev/null >/dev/null 2>&1 &
	warm_build=true
	status+=("background build of build/ started (log: build/warm-build.log)")
fi

report
if $warm_build; then
	echo "  Wait until build/warm-build.done exists (it holds the build's exit status) before building or testing in"
	echo "  build/; never run a second build in build/ at the same time. With a warm ccache this takes about a minute."
fi
echo "  Build: cmake --build build --target all openttd_test"
echo "  Test:  ctest --test-dir build -j4 --timeout 120"
exit 0

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository is

JGR's Patchpack (*jgrpp*): a heavily modified fork of OpenTTD (C++20, CMake). The main branch is `jgrpp`, which periodically merges upstream OpenTTD `master` ("Merge branch 'master' into jgrpp"). Much of `src/` is therefore upstream code with patchpack changes woven in; patchpack-only subsystems include template-based train replacement (`tbtr_*`), routing restrictions (`tracerestrict*`), programmable signals, plans, departure boards, scheduled dispatch, and variable day length.

Savegames written by jgrpp are not loadable by vanilla OpenTTD; jgrpp loads vanilla savegames up to the last merged upstream version.

## Build

```bash
./build.sh                # mkdir build, delete CMakeCache.txt, cmake .., make -j$(nproc)
./build-dedicated.sh      # same, with -DOPTION_DEDICATED=true (no GUI)
```

Manual equivalent (the default build type is Debug with asserts, which runs far slower than a release build):

```bash
mkdir build && cd build
cmake .. [-G Ninja] [-DCMAKE_BUILD_TYPE=RelWithDebInfo] [-DOPTION_DEDICATED=ON]
cmake --build . -j$(nproc)
```

- A GUI build fails at configure time without SDL2 or Allegro development headers ("SDL2 or Allegro is required for this platform"). In headless containers, configure with `-DOPTION_DEDICATED=ON`.
- Other libraries (lzma, zlib, png, zstd, lzo, curl, freetype, fontconfig, harfbuzz, icu, opus) are optional; see `COMPILING.md` and the apt list in `.github/workflows/ci-linux.yml`.
- Desync debugging: configure with `-DCMAKE_CXX_FLAGS_INIT="-DRANDOM_DEBUG"` (as the CI dedicated job does). Change `CXXFLAGS` only in a clean build directory, as they are cached.
- If GRFCodec/NFORenum are installed, the build may regenerate `.grf` files in the source tree. CI fails if a build or test run modifies tracked files (`git diff --exit-code`), so disable `GRFCODEC_EXECUTABLE`/`NFORENUM_EXECUTABLE` in the CMake cache if that happens.
- `make_bundle.sh` runs `cpack` in `build/`.

## Tests

The unit-test binary `openttd_test` (Catch2, sources in `src/tests/`) is `EXCLUDE_FROM_ALL`, so it must be built explicitly before `ctest`:

```bash
cmake --build . -j$(nproc) --target openttd openttd_test
ctest -j$(nproc) --timeout 120                 # what CI runs: unit tests + script regression tests
ctest -R 'FindLastBit'                         # one test, by regex over discovered names
./openttd_test "FindLastBit tests"             # one Catch2 TEST_CASE, run directly
./openttd_test --list-tests
```

- Each Catch2 `TEST_CASE` is registered as a separate ctest test via `catch_discover_tests`. New test files must be added to `add_test_files(...)` in `src/tests/CMakeLists.txt`.
- Script regression tests live in `regression/<name>/` (`main.nut`, `test.sav`, expected `result.txt`). They run the real `openttd` binary headlessly (`-x -snull -mnull -vnull:ticks=30000`) and compare script output with `result.txt`. Run them with `cmake --build . --target regression` (more verbose) or `ctest -R regression_`. CI installs the OpenGFX baseset into `~/.local/share/openttd/baseset` first.
- Other CI checks you can run locally: `python3 .github/unused-strings.py` and `python3 .github/script-missing-mode-enforcement.py` (script API functions that issue commands or read the company must call one of the `Enforce*Mode*` macros).

## Architecture

### Source layout and registration
Every source directory has a `CMakeLists.txt` that lists its files with `add_files(... [CONDITION ...])`; a new `.cpp`/`.h` is not compiled until it is listed there. Most top-level files in `src/` follow a naming scheme per subsystem: `foo_type.h` (types/enums), `foo_base.h` (pool object), `foo_func.h` (free functions), `foo_map.h` (tile accessors), `foo_cmd.cpp`/`foo_cmd.h` (commands and tile handlers), `foo_gui.cpp` (windows), with widget enums in `src/widgets/foo_widget.h`.

### Game state, commands and determinism
Multiplayer is lockstep: every client simulates the same game, so any change to game state must go through a command, and game-state code must be deterministic (use `Random()` for game state and `InteractiveRandom()` for anything else; never derive game state from GUI or client-local settings). See `docs/desync.md` and `docs/debugging_desyncs.md`.

jgrpp's command system differs from upstream's `DEF_CMD_TRAIT`:
- IDs are in `enum class Commands` (`src/command_type.h`).
- Each command is declared in its subsystem's `*_cmd.h` with `DEF_CMD_TUPLE(Commands::X, CmdProc, flags, CommandType::..., CmdDataT<...>)` and registered by including that header in `src/command_table.cpp`. `DEF_CMD_DIRECT*` passes a payload struct to the handler as `const T &` instead of packing a tuple. Suffix `_LT`: the tile is used only as a location (e.g. for error messages) and is not passed to the handler. Suffix `_NT`: no tile at all. The variants are documented above the macros in `command_type.h`.
- Callers use `Command<Commands::X>::Post([error_str,] [callback,] tile, args...)` for a networked, user-initiated command, or `Command<Commands::X>::Do(flags, tile, args...)` to test or execute from within another command.
Upstream patches that touch commands usually need adapting rather than cherry-picking.

### Map and tiles
Per-tile data lives in the map arrays and is read and written only through the `*_map.h` accessor functions. Behaviour per tile type is dispatched through `_tile_type_procs` (`TileTypeProcs` in `src/tile_cmd.h`, table in `src/landscape.cpp`), with handlers implemented in the corresponding `*_cmd.cpp` (`rail_cmd.cpp`, `road_cmd.cpp`, `station_cmd.cpp`, `tunnelbridge_cmd.cpp`, ...). The bit layout of the map is documented in `docs/landscape.html` and `docs/landscape_grid.html`. Game objects (vehicles, stations, towns, orders, etc.) are pool items (`src/core/pool_type.hpp`), accessed via `T::Get(id)`, `T::GetIfValid(id)` and `T::Iterate()`.

### Save/load (two loaders)
- `src/sl/` is jgrpp's save/load engine (file I/O, compression, versioning, chunk dispatch) and holds jgrpp's handlers for most chunks. Format changes are versioned with **extended feature versions** rather than by bumping the upstream `SLV_*` version: each feature is an `XSLFI_*` value in `src/sl/extended_ver_sl.h` (append new ones before `XSLFI_SIZE`), with its current version, flags and associated chunk IDs registered in `_sl_xv_sub_chunk_infos` in `src/sl/extended_ver_sl.cpp`. Fields are gated with `SLE_CONDVAR_X(..., SlXvFeatureTest(XSLFTO_AND, XSLFI_FOO, min_ver[, max_ver]))`, and code checks `SlXvIsFeaturePresent(XSLFI_FOO, ...)`. Post-load fixups go in `src/sl/afterload.cpp`. The SLXI chunk format and the rules for marking features ignorable or discardable are described at the top of `extended_ver_sl.cpp`.
- `src/saveload/` is upstream's table-based save/load code, in `namespace upstream_sl`, and largely tracks upstream merges. Some chunks (e.g. `ENGN`, `GOAL`, `GRPS`, `RAIL`, `ROAD`, `OBJS`, `SUBS`) are delegated to it wholesale via `MakeUpstreamChunkHandler<'XXXX', ...>`; others are saved in upstream format but can load either format depending on an `XSLFI_*` version (`MakeSaveUpstreamFeatureConditionalLoadUpstreamChunkHandler`). Table chunks are versioned by the upstream version recorded in SLXI (`_sl_xv_upstream_version`). Savegames from newer vanilla OpenTTD are loaded entirely through this code ("upstream mode", `_sl_upstream_mode`).
- Before changing a chunk, check its entry in the chunk-handler list at the bottom of the relevant `src/sl/*_sl.cpp` to see which of the two implementations actually handles it.

### Settings
Settings are declared in `src/table/settings/*.ini` and turned into C++ tables at build time by `settingsgen` (`src/settingsgen/`). Patchpack-only settings use `flags = SettingFlag::Patch` and `patxname = ""some.name""`, which stores them by name in the PATX chunk (tolerant of missing, extra or reordered settings) instead of the upstream PATS table; `extver` gates legacy savegame layouts. Their place in the settings window is defined in `GetSettingsTree()` in `src/settingentry_gui.cpp`.

### Strings and translations
`src/lang/english.txt` holds upstream strings; patchpack strings go in `src/lang/extra/english.txt`, which `strgen` merges in. Its directives include `##after STR_X` (insert after an upstream string), `##override on/off` (replace upstream text) and `##no-translate on/off`. Translations of patchpack strings live in `src/lang/extra/<language>.txt`; upstream translations arrive via upstream merges (Eints; see `docs/eints.md`), so do not hand-edit non-English upstream language files.

### Time
With variable day length, "ticks" are ambiguous. `StateTicks` (`_state_ticks`) is the monotonic game-state tick counter; dates are split into `EconTime::` and `CalTime::` types; `DayLengthFactor()` returns the current day length factor (`src/date_type.h`, `src/date_func.h`).

### Script and NewGRF extensions
- Script API: `src/script/api/script_*.hpp`; Squirrel bindings are generated from these headers at build time. API changes are logged in `ai_changelog.hpp`/`game_changelog.hpp`; jgrpp additions are documented in `docs/script-additions.html`.
- NewGRF: jgrpp extends the spec with feature testing and property, variable and feature-ID remapping (`src/newgrf_extension.cpp`), documented in `docs/newgrf-additions.html` and `docs/newgrf-additions-nml.html` (plus `newgrf-roadstops`, `newgrf-town`, `newgrf-newlandscape`).

### Patchpack documentation
Player-visible features are listed in `README.md`; release notes are in `jgrpp-changelog.md`; internal and performance changes are in `docs/jgrpp-low-level-changes.md`. `changelog.md` is upstream's. `.ottdrev-vc` is written by `version_utils.sh` during release tagging.

## Code style

Full rules: `CODINGSTYLE.md`. The points most often missed:
- Indent with tabs only; no trailing whitespace; C++ sources are ASCII only.
- Functions and types in `CamelCase`; variables in `snake_case`; globals start with `_`; refer to members as `this->member`.
- Function opening brace on its own line; control-statement braces on the same line; a single-statement `if` without `else` may go on one line without braces.
- Single-line comments use `/* */`; trailing `//` is for end-of-line comments only.
- Everything gets Doxygen (`/** ... */`, `///<` for members); every file starts with the GPL header block and a `/** @file name.cpp Description. */` comment.
- `CODINGSTYLE.md` specifies upstream's commit format (`Fix #123: [Component] Details`). jgrpp's own commits use plain imperative subjects, optionally with a component prefix (`Fix crash when ...`, `Departures: Fix column spacing ...`).

`CONTRIBUTING.md` is inherited from upstream OpenTTD, whose "Use of AI" policy forbids LLM-generated issues and pull requests to OpenTTD/OpenTTD.

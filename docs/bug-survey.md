# Survey of known and suspected C++ bugs

Surveyed on 2026-10-07, on top of jgrpp `6318727b0` (2026-10-04), against
upstream OpenTTD `master` at `0000a2acb` (2026-10-07).

The aim is a list of bugs to fix or to avoid reproducing while porting to Rust.
Each entry says how it was found and how sure the finding is.

## Sources and what they cover

| Source | Covered | Not covered |
| --- | --- | --- |
| Issue trackers of jgrpp and upstream | Nothing (see below) | All open issues |
| Upstream fixes not yet merged into jgrpp | All 28 `Fix` commits on upstream `master` that are not in jgrpp | Fixes still in open upstream pull requests |
| AddressSanitizer + UndefinedBehaviorSanitizer | Unit tests, script regression tests, five new games of one game year each, about 7 game years of the title game | Multiplayer, GUI interaction, NewGRFs, third-party AIs, large maps |
| `known-bugs.md` | Upstream's list of bugs it will not fix | |
| Code markers (`FIXME`, `XXX`, `TODO`, `HACK`) | All of `src/` except `3rdparty/` | |

### Issue trackers

Not read. This session can clone public repositories but cannot use the
GitHub API for repositories other than this fork, and attaching
`JGRennison/OpenTTD-patches` with API access was refused. To include them,
export the open issues with the GitHub CLI and commit the files (for example
under `docs/issues/`):

```bash
gh issue list -R JGRennison/OpenTTD-patches --state open --limit 2000 \
    --json number,title,labels,createdAt,updatedAt,body,url > jgrpp-issues-open.json
gh issue list -R OpenTTD/OpenTTD --state open --limit 2000 \
    --json number,title,labels,createdAt,updatedAt,body,url > openttd-issues-open.json
```

## Findings

Severity: **crash**, **desync** (clients diverge), **game** (wrong but
deterministic behaviour), **script** (values seen by AIs and game scripts),
**GUI**, **UB** (undefined behaviour without a known effect in practice).

### Upstream fixes not yet merged into jgrpp

Upstream `master` has 171 commits that jgrpp has not merged, 28 of them with
subjects starting with `Fix`. JGR cherry-picks many fixes ahead of a full
merge, so each was checked against this tree: first by applying the upstream
patch forwards and in reverse, then by reading the code where neither applies.

| Upstream commit | Subject | Status here | Severity |
| --- | --- | --- | --- |
| `5ac4eb48c` | Fix #11127: use callback for dual-headed engine purchase capacity | **Present.** `Engine::DetermineCapacity` (`src/engine.cpp:287`) adds the rear head's plain property instead of doubling the callback result. Shown in the purchase list and returned by `ScriptEngine::GetCapacity`. | script, GUI |
| `f58eac1d3` | Fix #15824: Game crash when truncating cargo of crashed vehicle | Not affected. The upstream crash is an underflow of the `Keep` count in `CargoRemoval<VehicleCargoList>`. jgrpp's `VehicleCargoList::Truncate` (`src/cargopacket.cpp:877`) calls `KeepAll()` first whenever more than the `Keep` count is removed, and `Truncate` is the only user of that action. | |
| `83d286383`, `122287992`, `3a33cd64c`, `ce14e19d8`, `8d9b24cc1` | Cargo truncation on capacity reduction; `BreakIterator::setText` temporary; autoclean; company colour order; bootstrap button size | Already fixed here (the upstream patches apply in reverse). | |
| `9060a4a7e`, `97951cfcf`, `252c239a1`, `4ecbb871d`, `d6848be64` | Earlier fixes from the August check | Already fixed here, with jgrpp's own code. | |
| `987fab4bd` | MinGW link fix for libsoxr | Not applicable (no libsoxr, no MinGW). | |
| `7b3743db1` | Grammar in the script API documentation | Not checked (documentation only). | |

| `14f7a28e8` | Fix f864aaf13: Fallback font choice should have all glyphs not only missing glyphs | **Present.** The fallback search tests only the missing glyphs (`src/os/unix/font_unix.cpp:199-206`, `src/os/macosx/font_osx.cpp:304, 316`), so a fallback can replace the configured font while lacking glyphs the configured font had. | GUI |
| `6a35a9718` | Fix: Pass full isocode when searching for font with fontconfig | **Present.** `src/os/unix/font_unix.cpp:160` cuts the isocode at `_`. | GUI |
| `117d5d695` | Fix: Widget sizes may be incorrect after font change due to language change | **Present.** `src/settings_gui.cpp:1575` re-initialises windows without recomputing font-dependent widget sizes. | GUI |
| `91e5f72f4` | Fix #15636: Allow shift and ctrl modifiers in hotkeys | **Present**, but see the note below before porting. | GUI |
| `a7755dbf7` | Fix: Wrong category for some base set debug messages | **Present** (`src/base_media_func.h:251`, `src/music.cpp:142, 148, 180`). | diagnostics |
| `4b5f010b4`, `2237b6b97` | Double-clicking a depot or waypoint order; glyphs for whitespace | Already fixed here, with jgrpp's own code. | |
| `5fcaa0982` | Fix #16053: Crash due to undefined behaviour when leaving screensaver mode | Not affected: jgrpp's game-tick timers are run from a copied vector (`src/timer/timer_game_tick.cpp:51`). Do not port the upstream change as written; see the note below. | |
| `58f8467ec`, `ee77792ed`, `ecbe2aa17`, `283849956`, `79e679604`, `1ea8a4cab`, `8cc483274`, `de306de89`, `141b4e5ee`, `e60411035` | Terraforming desync, reliability, elrail compatibility, crashed trains, station area, trees on coast, vehicle caches | Being checked. | |

Notes on these fixes:

- **Timers (`5fcaa0982`).** jgrpp keeps timers in a `btree::btree_set`
  (`src/timer/timer_manager.h:119`), where erasing invalidates all iterators,
  so upstream's fix (advance the iterator before calling the timer) would
  still be undefined behaviour here. Game-tick timers avoid this by iterating
  over a copy (`GetTimerVector()`), but a callback that deletes a *different*
  timer later in that copy would leave a dangling pointer. Real-time timers
  iterate over the set directly (`src/timer/timer_game_realtime.cpp:56`);
  this is safe only because no real-time timer callback currently adds or
  removes timers. Latent; worth fixing in the Rust port by design.
- **Hotkey modifiers (`91e5f72f4`).** Inferred from the code, not tested:
  jgrpp's polyline rail hotkeys (Ctrl+A, Ctrl+Shift+A, `src/rail_gui.cpp:1112`)
  toggle remove mode when Ctrl is pressed (`src/rail_gui.cpp:774-777`) and
  toggle back when it is released (`src/rail_gui.cpp:1055-1060`). If hotkeys
  run with Ctrl cleared, the first toggle is skipped but the second still
  happens, leaving the tool in remove mode. Check this when the change
  arrives with an upstream merge.

### Found by the sanitizers

The sanitizer build used clang 22 with `-fsanitize=address,undefined
-fno-omit-frame-pointer`, `-O2` and assertions on (`WITH_ASSERT`; the
debug-only `dbg_assert` checks were off). AddressSanitizer found no errors in
any run. UndefinedBehaviorSanitizer reports each source location once per
process, so the list below gives places, not frequencies.

| Location | Report | Severity | Assessment |
| --- | --- | --- | --- |
| `src/sl/saveload_buffer.h:151` (read) and `:344` (write) | Adds an offset to a null buffer pointer before the first block is acquired | UB | The comparison `bufp + bytes > bufe` with both pointers null is formally undefined; it works because the null case then acquires a buffer. Compare sizes (`bytes > bufe - bufp`) instead. |
| `src/sl/linkgraph_sl.cpp:210` | Rebases a settings `SaveLoad` address with `address -= offsetof(...); address += offsetof(...)`, passing through null | UB | The usual OpenTTD offset idiom; harmless in practice. A Rust port should describe fields by offset, not by fake pointers. |
| `src/pbs.cpp:531` via `pbs.cpp:268` | `flags &= ~FRF_TB_EXIT_FREE` stores `0xFFFFFFFD` in the unscoped enum `FollowReservationFlags`, outside its value range | UB | Harmless while `-fno-strict-enums` is set (`cmake/CompileFlags.cmake`, for GCC, Clang and AppleClang). Same pattern as the script API entries below. |
| `src/script/api/script_airport.cpp:25, 30, 57, 64, 71, 181` | Script passes an out-of-range `AirportType` (e.g. `-1`) that is converted to the enum before validation | UB | Harmless with `-fno-strict-enums`; the functions then reject the value. |
| `src/script/api/script_rail.cpp:75, 77, 508` | `ScriptRail::RailType` declares only `RAILTYPE_INVALID = -1`, so its value range is -1 to 0 and every rail type except the first is out of range | UB | As above; affects every script call that takes a rail type. |
| `src/script/api/script_order.cpp:210-222` via `script_order.hpp:609` | `~(OF_...)` on `ScriptOrderFlags` gives values outside the enum's range | UB | As above. |

For the Rust port: model flag sets as bit sets (as `EnumBitSet` already does in
newer C++ code) and convert script integers with a checked conversion that
fails, rather than casting to an enum.

### Upstream's list of unfixed bugs

`known-bugs.md` lists 28 bugs that upstream decided not to fix, mostly
platform and SDL problems. The ones that concern game logic, and so would
carry over into a Rust port unless changed on purpose:

- Lost trains ignore (block) exit signals (#1473).
- The owner of the last transfer leg is paid for the whole journey (#2427).
- "Forbid 90 degree turns" does not work for crossing PBS paths (#2737).
- Trains crash when entering the same junction from block and path signals (#3928).
- A train does not crash with itself (#4635).
- Aircraft move through walls in rotated airports (#4705).
- Vehicles do not keep their "maximum" speed (#4815).
- Trains might not stop at platforms that are being changed (#5553).
- Involuntary cargo exchange with cargodist via a neutral station (#6114).
- Incorrect ending year in the end-of-game newspaper (#8625).

These are upstream's issue numbers; whether jgrpp changed any of them was not
checked.

### Code markers

One `FIXME`, 94 `XXX`, 22 `TODO` and 3 `HACK` comments outside `3rdparty/`.
None of them describes a defect that can be acted on; most are notes about
style or design.

## Reproducing

Sanitizer build, in a separate build directory so that `build/` keeps its ccache hits:

```bash
cmake -S . -B build-asan -G Ninja -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
    -DCMAKE_C_FLAGS="-fsanitize=address,undefined -fno-omit-frame-pointer" \
    -DCMAKE_CXX_FLAGS="-fsanitize=address,undefined -fno-omit-frame-pointer" \
    -DCMAKE_EXE_LINKER_FLAGS="-fuse-ld=lld -fsanitize=address,undefined"
cmake --build build-asan --target all openttd_test
```

On Linux, the SDL2 library in the dev shell is `sdl2-compat`, which loads SDL3
with `dlopen`; under ASan that needs SDL3's library directory in
`LD_LIBRARY_PATH`. Soak run of the title game:

```bash
ASAN_OPTIONS=detect_leaks=0 UBSAN_OPTIONS=print_stacktrace=1:log_path=ubsan \
    ./build-asan/openttd -x -c soak.cfg -snull -mnull -vnull:ticks=200000
```

Upstream fixes not in jgrpp:

```bash
git fetch https://github.com/OpenTTD/OpenTTD master:refs/remotes/upstream/master
git fetch https://github.com/JGRennison/OpenTTD-patches jgrpp:refs/remotes/jgr/jgrpp
git log --format='%h %cs %s' refs/remotes/jgr/jgrpp..refs/remotes/upstream/master | grep ' Fix'
```

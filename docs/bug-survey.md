# Survey of known and suspected C++ bugs

Surveyed on 2026-10-07, on top of jgrpp `6318727b0` (2026-10-04), against
upstream OpenTTD `master` at `0000a2acb` (2026-10-07).

The aim is a list of bugs to fix or to avoid reproducing while porting to Rust.
Each entry says how it was found and how sure the finding is.

## Summary

Actionable, most important first:

1. **Fixed in this fork: desync from a console command** (upstream #12059):
   `setting_newgame` ran some settings' change callbacks against the running
   game, on the client that used it only.
2. **Fixed in this fork: cargo income arithmetic** (ported to Rust,
   `openttd_core::cargo_income`): large deliveries over long distances, and
   large deliveries with the profit callback, wrapped 32-bit products into
   wrong (often negative) incomes.
3. **`Money` arithmetic truncates 64-bit factors** (`OverflowSafeInt` in
   `src/core/overflowsafe_type.hpp`, same upstream): multiplying by an
   `int64_t`, a `uint` of 2^31 or more, or another `Money` cuts the factor
   to `int` before the overflow check, and dividing an `int64_t` by `Money`
   cuts the divisor. Some code depends on this by accident (see the profit
   callback above), so it can't simply be changed; a Rust port of money
   arithmetic must reproduce it or change it with every caller checked.
4. **Crashes reported in the trackers and still possible here:** a use after
   free in the story book when a game script changes pages (upstream
   #10566); an assertion after loading some savegames (upstream #14726); a
   YAPF assertion with jgrpp's reversing nodes (jgrpp #812, cause unknown).
5. **Fixed in this fork:** assertion failure in Debug builds when a crashed
   train whose last wagon is in a depot is cleared away
   (`DeleteLastWagon` in `src/train_cmd.cpp`). Present in latest jgrpp;
   upstream fixed its own variant in `1ea8a4cab`. Covered by the
   `regression_crashed_train_depot` test, which fails without the fix in
   builds with `dbg_assert`.
6. **Game logic present here and fixed upstream but not merged yet:** stale
   vehicle caches while loading (`e60411035`, affects station ratings);
   reliability above the model's (`ecbe2aa17`, affects breakdowns);
   dual-headed engines' purchase capacity (`5ac4eb48c`, also seen by AIs).
7. **Game logic present here and open upstream:** about twenty bugs, among
   them cargodist loading for a "no unloading" stop, a wrong timetable start
   that stalls trains in jgrpp, ships losing a moved buoy, trains colliding
   through depot walls, and the road pathfinder not costing its first tile.
   See "From the issue trackers".
8. **Savegames:** jgrpp 0.70 to 0.73.0 left wrong player-protection bits on
   town houses (jgrpp #996), and nothing clears them on load.
9. **Script-visible quirks** that scripts may rely on (listed under "Visible
   to scripts"); any fix or port must keep them or change them deliberately.
10. **Latent:** real-time timers iterate over a container that their
    callbacks must not change; game-tick timers iterate over a copy that can
    hold dangling pointers if a callback deletes another timer.
11. Smaller GUI and platform problems, in fonts, widget sizes, SDL2 on
    Wayland and X11, and macOS input.

When merging upstream: `91e5f72f4` (hotkey modifiers) probably breaks jgrpp's
polyline rail hotkeys, `5fcaa0982` (timers) does not fit jgrpp's timer
container, and `58f8467ec` and `de306de89` must come with the upstream commits
that caused their bugs.

The sanitizers found no memory errors; the undefined behaviour they found is
harmless with the current compiler flags but should not be carried into Rust.

## Sources and what they cover

| Source | Covered | Not covered |
| --- | --- | --- |
| Issue trackers of jgrpp and upstream | All 115 open jgrpp issues and 216 open upstream issues, as exported on 2026-10-07 (issue text only) | Comment threads, closed issues |
| Upstream fixes not yet merged into jgrpp | All 32 `Fix` commits on upstream `master` that are not in jgrpp | Fixes still in open upstream pull requests |
| AddressSanitizer + UndefinedBehaviorSanitizer | Unit tests, script regression tests, five new games of one game year each, about 7 game years of the title game | Multiplayer, GUI interaction, NewGRFs, third-party AIs, large maps |
| `known-bugs.md` | Upstream's list of bugs it will not fix | |
| Code markers (`FIXME`, `XXX`, `TODO`, `HACK`) | All of `src/` except `3rdparty/` | |

### Issue trackers

Exported with the GitHub CLI on 2026-10-07 (title, labels, dates and
description; no comments):

```bash
gh issue list -R JGRennison/OpenTTD-patches --state open --limit 2000 \
    --json number,title,labels,createdAt,updatedAt,body,url > jgrpp-issues-open.json
gh issue list -R OpenTTD/OpenTTD --state open --limit 2000 \
    --json number,title,labels,createdAt,updatedAt,body,url > openttd-issues-open.json
```

Each issue was classified, and each bug that can affect game state on Linux
or macOS was looked up in this tree. Because the comment threads were not
exported, a bug may have been diagnosed or worked around in discussion
without that showing here.

| Tracker | Open | Bugs | Feature requests | Questions | Other |
| --- | --- | --- | --- | --- | --- |
| jgrpp | 115 | 21 | 85 | 2 | 7 |
| upstream | 216 | 168 | 35 | 6 | 7 |

## Findings

Severity: **crash**, **desync** (clients diverge), **game** (wrong but
deterministic behaviour), **script** (values seen by AIs and game scripts),
**GUI**, **UB** (undefined behaviour without a known effect in practice).

### Upstream fixes not yet merged into jgrpp

Upstream `master` has 187 commits (from 2026-07-26 on) that jgrpp has not
merged, 32 of them with subjects starting with `Fix`. JGR cherry-picks many
fixes ahead of a full merge, so each was checked against this tree: first by
applying the upstream patch forwards and in reverse, then by reading the code
where neither applies.

| Upstream commit | Subject | Status here | Severity |
| --- | --- | --- | --- |
| `5ac4eb48c` | Fix #11127: use callback for dual-headed engine purchase capacity | **Present.** `Engine::DetermineCapacity` (`src/engine.cpp:287`) adds the rear head's plain property instead of doubling the callback result. Shown in the purchase list and returned by `ScriptEngine::GetCapacity`. | script, GUI |
| `f58eac1d3` | Fix #15824: Game crash when truncating cargo of crashed vehicle | Not affected. The upstream crash is an underflow of the `Keep` count in `CargoRemoval<VehicleCargoList>`. jgrpp's `VehicleCargoList::Truncate` (`src/cargopacket.cpp:877`) calls `KeepAll()` first whenever more than the `Keep` count is removed, and `Truncate` is the only user of that action. | |
| `83d286383`, `122287992`, `3a33cd64c`, `ce14e19d8`, `8d9b24cc1` | Cargo truncation on capacity reduction; `BreakIterator::setText` temporary; autoclean; company colour order; bootstrap button size | Already fixed here (the upstream patches apply in reverse). | |
| `9060a4a7e`, `97951cfcf`, `252c239a1`, `4ecbb871d`, `d6848be64` | Dual-headed capacity when sorting; rivers and small seas; script string saving; house protection; NewGRF label byte order | Already fixed here (cherry-picked by JGR). | |
| `987fab4bd` | MinGW link fix for libsoxr | Not applicable (no libsoxr, no MinGW). | |
| `7b3743db1` | Grammar in the script API documentation | Not checked (documentation only). | |
| `14f7a28e8` | Fix f864aaf13: Fallback font choice should have all glyphs not only missing glyphs | **Present.** The fallback search tests only the missing glyphs (`src/os/unix/font_unix.cpp:199-206`, `src/os/macosx/font_osx.cpp:304, 316`), so a fallback can replace the configured font while lacking glyphs the configured font had. | GUI |
| `6a35a9718` | Fix: Pass full isocode when searching for font with fontconfig | **Present.** `src/os/unix/font_unix.cpp:160` cuts the isocode at `_`. | GUI |
| `117d5d695` | Fix: Widget sizes may be incorrect after font change due to language change | **Present.** `src/settings_gui.cpp:1575` re-initialises windows without recomputing font-dependent widget sizes. | GUI |
| `91e5f72f4` | Fix #15636: Allow shift and ctrl modifiers in hotkeys | **Present**, but see the note below before porting. | GUI |
| `a7755dbf7` | Fix: Wrong category for some base set debug messages | **Present** (`src/base_media_func.h:251`, `src/music.cpp:142, 148, 180`). | diagnostics |
| `4b5f010b4`, `2237b6b97` | Double-clicking a depot or waypoint order; glyphs for whitespace | Already fixed here, with jgrpp's own code. | |
| `5fcaa0982` | Fix #16053: Crash due to undefined behaviour when leaving screensaver mode | Not affected: jgrpp's game-tick timers are run from a copied vector (`src/timer/timer_game_tick.cpp:51`). Do not port the upstream change as written; see the note below. | |
| `1ea8a4cab` | Fix #15990, c9cf1418139: Game crash when clearing up crashed train with last wagon in depot | **Fixed in this fork** (was present in jgrpp's own form). Upstream's cause is not here, but `GetTrackbitsFromCrashedVehicle` returns `TRACK_BIT_NONE` for a wagon in a depot (`src/train_cmd.cpp:6441`), and `DeleteLastWagon` then calls `TrackBitsToTrack(TRACK_BIT_NONE)` (`src/train_cmd.cpp:6521`). That fails a `dbg_assert` (`src/track_func.h:195`), which is compiled only in Debug builds or with `OPTION_DBG_ASSERTS` (in CI, only the macOS Debug job). Other builds are unaffected (the result is not used). Latest jgrpp has the same code. Fix: compute the track only when `trackbits != TRACK_BIT_NONE`. | crash (Debug builds) |
| `ecbe2aa17` | Fix: Limit vehicle reliability to the model's current maximum reliability | **Present.** `src/vehicle.cpp:2304` clamps the decreased reliability at 0 but not at the engine's current reliability. jgrpp's `reliability_decay_speed` setting and the cases without decay make it likelier that a vehicle stays above its model. | game |
| `e60411035` | Fix: invalidate/update vehicle caches when vehicle is marked dirty | **Present.** `Train::MarkDirty` (`src/train_cmd.cpp:4981`) only calls `CargoChanged()`; the road vehicle and aircraft versions refresh no caches. `cached_max_speed` can therefore be stale while loading, and `src/economy.cpp:2202-2217` uses it for the station rating. Not a desync: jgrpp's VENC chunk sends the server's cached values to joining clients. A port would use jgrpp's `ConsistChanged(CCF_LOADUNLOAD)`. | game |
| `ee77792ed` | Fix: Terrain average height setting only applies to TerraGenesis | **Present.** `src/genworld_gui.cpp:607-609` leaves the average height dropdown enabled for the original generator, which ignores it (`src/tgp.cpp:548` is the only reader). | GUI |
| `283849956`, `79e679604`, `8cc483274`, `141b4e5ee` | Electric rail compatibility when disabled; signed `-1` in script returns; reservation under crashed trains; trees on coast rocks | Already fixed here (cherry-picked by JGR). | |
| `58f8467ec`, `de306de89` | Fix #16018 (desync when terraforming), Fix #15985 (station area after adding to a station) | Not applicable yet: the bugs come from upstream `2692444f4` and `dc39225d6`, which jgrpp has not merged. Take the fixes together with those commits, or the merge brings the bugs in. | (desync, game) |

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

### From the issue trackers

"jgrpp" and "upstream" give the tracker; numbers are issue numbers there.
Status: **present** means the faulty code was found in this tree;
**unclear** means it was looked for but the cause was not found;
**(re-checked)** marks findings that were checked a second time
independently. Bugs that only affect Windows, Emscripten or a GUI detail are
counted above but not listed.

#### Desync and determinism

| Issue | Problem | Status here |
| --- | --- | --- |
| upstream #12059 | The console command `setting_newgame` ran the setting's post-change callback during a game. jgrpp already applies such changes in a temporary main-menu mode, so callbacks that check the game mode (such as the one for `freeform_edges`) were safe, but about a dozen others changed the running game: for example the engine lifetime settings re-ran `StartupEngines()`, `train_speed_adaptation` cleared all signal speed restrictions, the braking model reset brake heat, `disable_elrails` changed rail compatibility, and the aircraft range and breakdown settings reset vehicle flags. A client running the command desynced. | **Fixed in this fork**: callbacks and the game log now react only to changes of the settings in use (`IsChangeOfSettingsInUse` in `src/settings.cpp`); unit tests in `src/tests/settings_newgame.cpp`. |
| upstream #9079 | NewGRFs reading lazily filled vehicle caches can desync. | Unclear. Same cache design (`src/newgrf_engine.cpp:560-762`); jgrpp's cache check (`src/cachecheck.cpp:419-447`) would detect mismatches. |
| upstream #15552 | Repeated desyncs on one server. | Unclear; little information. |

#### Crashes on Linux and macOS

| Issue | Problem | Status here |
| --- | --- | --- |
| upstream #10566 | Use after free in the story book window when a game script changes pages: the window keeps raw `StoryPage` pointers and rebuilds its list only during window updates, after input has been handled. | Present (`src/story_gui.cpp:43, 258, 860`). |
| upstream #14726 | Assertion `load_unload_ticks != 0` after loading a savegame. | Same assertion (`src/economy.cpp:2450`) and no fix-up on load; cause of the state unknown. Relevant to loading vanilla saves. |
| jgrpp #812 | YAPF assertion `n.estimate >= n.parent->estimate` (`src/pathfinder/yapf/yapf_destrail.hpp:204`). | Unclear. Possibly jgrpp's reverse-behind-signal nodes and their cost; root cause not found. The only open game-logic crash in jgrpp's tracker. |
| upstream #11166 | Crash on a key press in the macOS text input code. | Unclear. The Cocoa callbacks dereference `GetFocusedTextbuf()` (`src/video/cocoa/cocoa_wnd.mm:1025-1173`), but each first checks `EditBoxInGlobalFocus()`, which rules out a null buffer in the normal case; cause not found. |
| upstream #9691 | A client connecting over IPv6 through TURN crashes an IPv4-only server. | Unclear; `NOT_REACHED` in address-family switches remains (`src/network/core/address.cpp`). |

#### Savegames

| Issue | Problem | Status here |
| --- | --- | --- |
| jgrpp #996 | Town growth built player-protected houses. | Fixed (cherry-pick of upstream's fix), but saves from jgrpp 0.70 to 0.73.0 keep the wrong protection bits (`m3` bit 5) on houses, and nothing clears them on load. |
| upstream #15139 | "Invalid town name generator" when loading an old save. | Unclear without the file. |

#### Game logic

| Issue | Problem | Status here |
| --- | --- | --- |
| none | Cargo income from the profit callback: `result * num_pieces` multiplies a signed `int` by an unsigned `uint`, so the product wraps around as an unsigned 32-bit value. Negative multipliers still came out right, because `Money`'s multiplication truncates its factor to `int` (see the summary), but products beyond 2^31 (from about 131,000 units at the largest multiplier) gave wrong incomes. | **Fixed in this fork**, in the Rust port of the income arithmetic (`openttd_core::cargo_income`): computed in 64 bits; tests in Rust and in `src/tests/cargo_income.cpp`. |
| none (related to upstream #9719) | Cargo income: `dist * time_factor * num_pieces` was computed in 32 bits and passed to an `int32_t`, so it wrapped for large deliveries over long distances (for example 2,100 units over 4,200 tiles at the highest time factor of 255), and the result was cut to 32 bits as well. jgrpp's large maps and long trains make this easier to reach. | **Fixed in this fork**, in the same Rust port: computed in 64 bits with a saturating multiplication; a differential test against the original C++ shows identical results wherever the original did not overflow. |
| upstream #14734 | Cargodist loads cargo for a stop that has "no unloading". | Likely present in jgrpp's own order prediction (`src/order_cmd.cpp:600-608`). |
| upstream #12980 | Timetable start is detected wrongly when the first manual order is reached; the reporter saw trains stall in jgrpp. | Present (`src/timetable_cmd.cpp:918-930`). |
| upstream #12301 | A ship gets lost when a buoy is moved: the reused buoy gets a new location but orders keep the old destination tile. | Present (`src/waypoint_cmd.cpp:541-544`, `src/order_cmd.cpp:4280`). |
| upstream #8424 | Trains collide through the back wall of a depot: the collision test uses distance only. | Present (`src/train_cmd.cpp:5272-5312`). |
| upstream #15254 | A blocked road vehicle copies the speed of the vehicle in front without limiting it to its own maximum. | Present (`src/roadveh_cmd.cpp:1702, 1843, 1959, 2013`). |
| upstream #15701 | The road pathfinder never charges the cost of its first tile after a depot or stop. | Present (`src/pathfinder/yapf/yapf_road.cpp:400-408`). |
| upstream #15000 | The interest rate set in the scenario editor is not the one charged; the finance window shows the setting. | Present (`src/economy.cpp:1025`, `src/company_gui.cpp:390`). |
| upstream #14624 | Without a configuration file, the link graph defaults are doubled. | Present (`src/settings.cpp:1279, 1526-1529`). |
| upstream #11392 | Exclusive transport rights ignore neutral stations such as oil rigs. Visible to game scripts. | Present (`src/economy.cpp:1161`, `src/station_cmd.cpp:5118`). |
| upstream #12891 | Timetable speeds entered by the player are stored one lower: input is rounded and display truncated, with `double` factors. | Present (`src/strings.cpp:1163-1177`). |
| jgrpp #797 | Public road generation silently fails to connect to town roads of another road type, because a failed build is ignored. | Present (`src/road.cpp:808-823`). |
| jgrpp #1004 | Long trains "too heavy" and slow since 0.72.0, probably related to trains driving backwards. | Unclear; root cause not found. |
| upstream #8048 | A path reservation can survive a train crash. | Unclear. |
| upstream #6503, #10193 | Trains jump by part of a tile when reversing at a line end; articulated trains shift. | Same code as upstream. |
| upstream #8022, #12193, #12333, #13164 | Ship pathfinding: service loop, docking tile choice, dead ends after terraforming, detours. | Same code as upstream. |
| upstream #14387, #15020, #15774, #16016, #8088, #15021 | Map generation: no farms (or no fields) near a low snow line; no desert or snow below 50% on flat maps; town count silently limited by available names; the scenario editor never advances `_tick_counter`, so trees there stay saplings. | Present. |

#### Visible to scripts

These are present and long-standing; AIs and game scripts may depend on
them, so a fix or a port must keep them or change them deliberately.

- upstream #5283: script commands ignore pause in single player but wait for unpause in multiplayer (`src/command.cpp:493-498`, `src/network/network_command.cpp:185`).
- upstream #15893: calling an API function through `acall()` cannot issue commands (`src/script/api/script_object.cpp:271`).
- upstream #14573: for trains, "no route to depot" is reported as `ERR_UNKNOWN` (`src/script/api/script_vehicle.hpp:46`).
- upstream #16067: `IsBuildable` is true on a one-piece town road (`src/script/api/script_tile.cpp:42-44`).
- upstream #10156: with `ai_developer_tools`, script settings look editable but cannot be changed (`src/script/script_gui.cpp:561-573`).
- jgrpp #656: in wallclock games, the economy year starts at 1920 in new jgrpp games but at 1 in vanilla, and `GSDate` returns it unadjusted. Deliberate, but a port of the date code has to keep both.

#### Linux and macOS front end

- jgrpp #478: the Linux music driver here is extmidi (the dev shell has no FluidSynth); a player that exits at once still makes the playlist skip quickly.
- upstream #12691 (Wayland mouse stays grabbed), jgrpp #818 (multi-monitor full screen on X11): present in the SDL2 driver, same as upstream.
- upstream #10083: the cheats hotkey (Ctrl+Alt+C) cannot be typed on macOS, where Cmd maps to Meta.
- upstream #15316 (cursor unusable on macOS 26.3), jgrpp #589 (cursor not updated on macOS), jgrpp #318 (repeated permission prompts for Documents, probably code signing): not checked.

#### Fixed in jgrpp, still open upstream

Keep jgrpp's version when merging upstream or porting: upstream #6618, #10028,
#10132, #10948, #11034, #12128, #12651, #15098, #15178, #15556.

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

In a shallow clone, fetch both with the clone's own `--shallow-since` date
or earlier. A later boundary applies to every ref and hides older unmerged
commits from the range.

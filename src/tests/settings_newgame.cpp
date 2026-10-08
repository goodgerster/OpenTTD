/*
 * This file is part of OpenTTD.
 * OpenTTD is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 2.
 * OpenTTD is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
 * See the GNU General Public License for more details. You should have received a copy of the GNU General Public License along with OpenTTD. If not, see <https://www.gnu.org/licenses/old-licenses/gpl-2.0>.
 */

/** @file settings_newgame.cpp Tests for changing the settings for new games. */

#include "../stdafx.h"

#include "../3rdparty/catch2/catch.hpp"

#include "../openttd.h"
#include "../settings_func.h"
#include "../settings_type.h"
#include "../train_speed_adaptation.h"

#include "../safeguards.h"

/*
 * The post-change callback of vehicle.train_speed_adaptation clears the signal
 * speed restrictions of the running game, which is enough to see whether it ran.
 */

static void AddSignalSpeedRestriction()
{
	_signal_speeds.clear();
	_signal_speeds[SignalSpeedKey{ TileIndex{1}, 0, TRACKDIR_X_NE }] = SignalSpeedValue{ 100, StateTicks{0} };
}

TEST_CASE("setting_newgame during a game leaves the running game alone")
{
	const GameMode old_game_mode = _game_mode;
	_game_mode = GameMode::Normal;

	_settings_game.vehicle.train_speed_adaptation = false;
	_settings_newgame.vehicle.train_speed_adaptation = false;
	AddSignalSpeedRestriction();

	IConsoleSetSetting("vehicle.train_speed_adaptation", "true", true);

	CHECK(_settings_newgame.vehicle.train_speed_adaptation == true);
	CHECK(_settings_game.vehicle.train_speed_adaptation == false);
	CHECK(_signal_speeds.size() == 1);
	CHECK(_game_mode == GameMode::Normal);

	_signal_speeds.clear();
	_game_mode = old_game_mode;
}

TEST_CASE("setting_newgame in the main menu still runs the callback")
{
	const GameMode old_game_mode = _game_mode;
	_game_mode = GameMode::Menu;

	_settings_newgame.vehicle.train_speed_adaptation = false;
	AddSignalSpeedRestriction();

	IConsoleSetSetting("vehicle.train_speed_adaptation", "true", true);

	CHECK(_settings_newgame.vehicle.train_speed_adaptation == true);
	CHECK(_signal_speeds.empty());

	_game_mode = old_game_mode;
}

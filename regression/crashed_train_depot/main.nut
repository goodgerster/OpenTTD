/*
 * test.sav was made by running this script in a new game, with a temporary
 * change to the game that crashes a train as soon as it starts leaving its
 * depot. The save holds the crashed train, whose last wagons are still in the
 * depot, before the game clears it away. Loading the save runs the check.
 */

class CrashedTrainDepot extends AIController {
	function Start();
	function Build();
	function TryBuildAt(x, y);
	function Check();
};

function CrashedTrainDepot::Start()
{
	if (AIDepotList(AITile.TRANSPORT_RAIL).Count() == 0) {
		this.Build();
		while (true) this.Sleep(1000);
	}
	this.Check();
	while (true) this.Sleep(1000);
}

function CrashedTrainDepot::Check()
{
	print("--CrashedTrainDepot--");
	print("  Rail depots: " + AIDepotList(AITile.TRANSPORT_RAIL).Count());
	print("  Vehicles before clean-up: " + AIVehicleList().Count());
	/* The game starts clearing a crashed train away 4440 ticks after the crash,
	 * about 2200 ticks after the save was made. Sleep counts AI iterations,
	 * which at the default competitor speed are 4 ticks apart. */
	this.Sleep(1500);
	print("  Vehicles after clean-up: " + AIVehicleList().Count());
}

function CrashedTrainDepot::Build()
{
	AICompany.SetLoanAmount(AICompany.GetMaxLoanAmount());
	AIRail.SetCurrentRailType(AIRailTypeList().Begin());
	for (local y = 4; y < AIMap.GetMapSizeY() - 4; y++) {
		for (local x = 4; x < AIMap.GetMapSizeX() - 12; x++) {
			if (this.TryBuildAt(x, y)) return;
		}
	}
	print("No place to build");
}

function CrashedTrainDepot::TryBuildAt(x, y)
{
	for (local i = 0; i < 8; i++) {
		local tile = AIMap.GetTileIndex(x + i, y);
		if (!AITile.IsBuildable(tile) || AITile.GetSlope(tile) != AITile.SLOPE_FLAT) return false;
	}

	local depot = AIMap.GetTileIndex(x, y);
	if (!AIRail.BuildRailDepot(depot, AIMap.GetTileIndex(x + 1, y))) return false;
	for (local i = 1; i < 8; i++) {
		if (!AIRail.BuildRailTrack(AIMap.GetTileIndex(x + i, y), AIRail.RAILTRACK_NE_SW)) return false;
	}

	local rail_type = AIRail.GetCurrentRailType();
	local engine = null;
	local wagon = null;
	foreach (e, _ in AIEngineList(AIVehicle.VT_RAIL)) {
		if (!AIEngine.IsBuildable(e) || !AIEngine.CanRunOnRail(e, rail_type)) continue;
		if (AIEngine.IsWagon(e)) {
			if (wagon == null) wagon = e;
		} else if (engine == null && AIEngine.HasPowerOnRail(e, rail_type)) {
			engine = e;
		}
	}

	local train = AIVehicle.BuildVehicle(depot, engine);
	for (local i = 0; i < 4; i++) {
		local w = AIVehicle.BuildVehicle(depot, wagon);
		AIVehicle.MoveWagon(w, 0, train, AIVehicle.GetNumWagons(train) - 1);
	}
	print("Built train " + train + " with " + AIVehicle.GetNumWagons(train) + " parts at depot " + depot);
	AIVehicle.StartStopVehicle(train);
	return true;
}

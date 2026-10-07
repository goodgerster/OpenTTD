class CrashedTrainDepot extends AIInfo {
	function GetAuthor()      { return "OpenTTD fork developers"; }
	function GetName()        { return "CrashedTrainDepot"; }
	function GetShortName()   { return "RGCT"; }
	function GetDescription() { return "Regression test: a crashed train whose last wagon is in a depot is cleared away."; }
	function GetVersion()     { return 1; }
	function GetAPIVersion()  { return "16"; }
	function GetDate()        { return "2026-10-07"; }
	function CreateInstance() { return "CrashedTrainDepot"; }
	function UseAsRandomAI()  { return false; }
}

RegisterAI(CrashedTrainDepot());

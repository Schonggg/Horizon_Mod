# Auto Fishing

A client-only Horizon native mod. Press **F8** to toggle automatic fishing.

1. Equip a fishing rod in the main hand.
2. Press F8 and cast once manually.
3. The mod reels in when the server reports a bite, then casts again after the bobber disappears.
4. Press F8 again to stop.

It reads the local client's fishing-bobber metadata and sends ordinary `UseItem` packets. It does not change server
logic or bypass server protections. Server rules may prohibit automated play.

Build the source folder directly into the selected Horizon instance:

```powershell
Set-Location .\auto-fishing
horizon-launcher native-build . --instance <instance> --trust-native-code
```

This builds and selects a native game version for that instance; it does not create an `.hnmod` file. The packaged
`auto_fishing.hnmod` is a separate bundle. To install that bundle for an instance, place it in the instance's
`horizon-native-mods` folder, then run `horizon-launcher native-build --instance <instance> --trust-native-code`.
That builds all native mods installed in the folder.

Native mods run with the player's rights; only install this mod if you trust its source.

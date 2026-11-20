# Auto Fishing

Press **F8** to enable or disable it; a status message appears in local chat. Then cast a fishing rod normally.
When the fishing bobber splash sound plays nearby, the mod automatically reels in and recasts after a short delay.

This is a client-only mod; install it only on the client. It uses the vanilla splash sound instead of custom
network payloads. Since Horizon's client API does not expose which bobber produced the sound, another
player's bobber within 32 blocks can also trigger a reel while Auto Fishing is armed.

## Build

The mod is built for Horizon 26.3 using the native client mod API. Build and install it with the Horizon launcher:

```powershell
horizon-launcher native-build . --instance <instance> --trust-native-code
```

To package the flat `.hmod` for distribution, use [`cargo horizon-mod`](https://github.com/horizon-rust/horizon):

```powershell
cargo test
cargo horizon-mod build
cargo horizon-mod check target/horizon-mod/auto_fishing.hmod
```

The package contains the root `horizon-mod.toml`, `Cargo.toml`, and `src/` files required by the launcher.

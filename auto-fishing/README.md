# Auto Fishing

Press **F8** to enable or disable it. Cast the fishing rod normally. When the server reports a bite, the mod
automatically reels in and recasts after a short delay.

## Build

The mod is built for Horizon 26.3 using the native mod API. Build and install it with the Horizon launcher:

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

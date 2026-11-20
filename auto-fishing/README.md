# Auto Fishing

A Horizon 0.2 WebAssembly mod for automatic fishing.

Press **F8** to enable or disable it. Cast the fishing rod normally. When the server reports a bite, the mod
automatically reels in and recasts after a short delay.

## Build

With [`cargo horizon-mod`](https://github.com/horizon-rust/horizon):

```powershell
cargo test
cargo horizon-mod build --wasm
cargo horizon-mod check
```

The build produces `target/horizon-mod/auto_fishing.hmod`.

# Arteries prototype

Initial Phase 1 prototype for the transport-economy game, written in Diamond.

Run:

```sh
/home/yria/dev/Diamond/target/debug/diamond check /home/yria/dev/arteries/sim.dia
/home/yria/dev/Diamond/target/debug/diamond run /home/yria/dev/arteries/sim.dia
```

The program simulates four settlements for 12 ticks in two scenarios: isolated
(no links) and connected (all links). It prints population, stockpiles,
prosperity, and research for each settlement, so the effect of transport links is
visible and explainable.

## Web prototype

```sh
/home/yria/dev/Diamond/target/debug/diamond check app/diamond.toml
/home/yria/dev/Diamond/target/debug/diamond build app/diamond.toml --out .diamond/app-build
/home/yria/dev/Diamond/target/debug/diamond serve .diamond/app-build --listen 127.0.0.1:3010
# then open http://127.0.0.1:3010/ (plain HTTP, not HTTPS)
```

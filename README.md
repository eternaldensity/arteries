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

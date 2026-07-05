# LifelineMesh

LifelineMesh is a Rust library that decodes an offline disaster-response mesh protocol used by field relays, shelters, mobile clinics, supply depots, and route planners.

The format is intentionally multi-stage: wire frames carry TLV payloads, fragments can be reassembled, dictionaries define phrase aliases, templates describe supply manifests, route updates publish constrained waypoint graphs, and script frames model field automation playbooks. The fuzzing harnesses exercise the same stateful decoder paths as a real ingest service.

This repository is self-contained for Fenrir/ClusterFuzzLite submission:

- first-party Rust source lives under `src/`
- cargo-fuzz style harnesses live under `fuzz/fuzz_targets/`
- seed corpus files live under `fuzz/corpus/`
- `.clusterfuzzlite/build.sh` builds every harness into `$OUT` without network access

The library intentionally contains deep lifecycle memory-safety defects in unsafe cache reuse paths. They are meant for fuzzing challenge tasks and should be patched by fixing the affected ownership/lifetime model, not by disabling harness coverage.

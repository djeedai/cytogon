# 🦠 Cytogon

![License: MIT or Apache 2.0](https://img.shields.io/badge/License-MIT%20or%20Apache2-blue.svg)
[![Doc](https://docs.rs/cytogon/badge.svg)](https://docs.rs/cytogon)
[![Crate](https://img.shields.io/crates/v/cytogon.svg)](https://crates.io/crates/cytogon)

🦠 Cytogon — A mesh generation library based on cellular automata.

## Overview

🦠 Cytogon allows generating 2D and 3D meshes from cellular automata.

```rust
// Create a 128³ grid and randomly fill it at 60%
let mut cave = Grid3::new(UVec3::new(128, 128, 128));
cave.fill_rand(0.6, rand::thread_rng());

// CA rule: 13-26/13-14,17-19/2/M
let rule = Rule3 {
    birth: RuleBitset3::from(13u8..=14u8) | (17u8..=19u8).into(),
    survive: (13u8..=26u8).into(),
}; // == Rule3::SMOOTH

// Iteratively apply the cellular automaton rule 5 times
for _ in 0..5 {
    cave.smooth(&rule);
}
```

For a viewer (GUI), see the [🔬 μscope](https://github.com/djeedai/cytogon/tree/main/uscope) repository ([📦`uscope`](https://crates.io/crates/uscope) crate).

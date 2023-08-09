# EphemerOpt

An ephemeral `Option` for Rust. When created, this `EphemeralOption` takes an expiry time and a value, and the `EphemeralOption` will revert to `None` after the time runs out.

This can be useful for possibly caching values instead of rerunning an expensive computation to get them. See the examples for a real-world demonstration of this using CPU data and message passing.

**NOTE**: This crate does use `unsafe`. If this bothers you, either don't use it or audit the code yourself. It is relatively straightforward, so this should be easy to do. Regardless, the tests pass `miri`, and everything is carefully tracked through the program, so there shouldn't be any undefined behavior.

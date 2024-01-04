# EphemerOpt

An ephemeral `Option` for Rust. When created, this `EphemeralOption` takes an expiry time and a value, and the `EphemeralOption` will revert to `None` after the time runs out.

This can be useful for possibly caching values instead of rerunning an expensive computation to get them. See the examples for a real-world demonstration of this using CPU data and message passing.

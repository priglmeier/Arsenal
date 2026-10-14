//! Build identification shown in Help > About Arsenal.

/// Unix time (seconds since 1970-01-01 UTC) of the last change to the source.
/// It is updated with every change; the version number in Cargo.toml only when asked.
pub const BUILD_EPOCH: u64 = 1791992987;

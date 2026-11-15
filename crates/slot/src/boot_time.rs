use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

pub fn mark(phase: &str) {
    eprintln!("slot: boot: {phase} at {:.1} ms", ms());
}

/// The build a log came from, printed once so a card's log can never be attributed to the wrong
/// binary while chasing a bug that only shows up on the device.
pub fn identify() {
    let build = crate::build_info::Build::current();
    eprintln!(
        "slot: build: {} ({}), built {}",
        build.version, build.hash, build.date
    );
}

pub fn ms() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
}

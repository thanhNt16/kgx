use std::time::Instant;
use kgx_core::json::JsonEnvelope;

pub fn emit<T: serde::Serialize>(
    command: &str,
    data: T,
    json: bool,
    start: Instant,
    human: impl FnOnce(&T),
) {
    let elapsed = start.elapsed().as_millis() as u64;
    if json {
        let env = JsonEnvelope::success(command, &data, elapsed);
        println!(
            "{}",
            serde_json::to_string_pretty(&env).expect("serialize envelope")
        );
    } else {
        human(&data);
    }
}

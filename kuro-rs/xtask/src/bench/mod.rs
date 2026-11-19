mod route_micro;
mod stats;

pub fn run(args: &[String]) -> i32 {
    let kind = args.first().map(String::as_str).unwrap_or("router");
    match kind {
        "router" | "route" => route_micro::run(),
        "stt" | "tts" | "turn" => {
            eprintln!("bench: {kind} not implemented yet; use `cargo xtask bench router`");
            0
        }
        other => {
            eprintln!("bench: unknown target {other:?}; try router|stt|tts|turn");
            2
        }
    }
}

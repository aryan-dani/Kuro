mod bench;
mod check;
mod gate;
mod generate;
mod new_adr;
mod new_feature;
mod paths;

use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = match args.as_slice() {
        [a, b] if a == "check" && b == "size" => check::size::run(),
        [a, b] if a == "check" && b == "text" => check::text::run(),
        [a, b] if a == "check" && b == "arch" => check::arch::run(),
        [a, b, c, d] if a == "check" && b == "protected" && c == "--base" => check::protected::run(d),
        [a, b] if a == "gate" && b == "fast" => gate::run_fast(),
        [a, b] if a == "gate" && b == "full" => gate::run_full(),
        [a, b] if (a == "new-feature" || a == "new-tool") && !b.is_empty() => new_feature::run(b),
        [a, rest @ ..] if a == "new-adr" && !rest.is_empty() => {
            let title = rest.join(" ");
            new_adr::run(&title)
        }
        [a] if a == "gen" => generate::run(),
        [a, b] if a == "gen" && b == "check" => generate::run_check(),
        [a] if a == "bench" => bench::run(&[]),
        [a, rest @ ..] if a == "bench" => bench::run(rest),
        _ => {
            eprintln!(
                "usage: xtask check size|text|arch|protected --base <ref> | gate fast|full | gen [check] | bench [router|stt|tts|turn] | new-feature <id> | new-adr \"<title>\""
            );
            2
        }
    };
    std::process::exit(code);
}

mod doctor;
mod init;

use std::env;
use std::process::ExitCode;

use doctor::{DoctorContext, default_context, doctor_json, print_human, run_doctor, rustc_version_line};
use init::{InitError, init_defaults};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("init") => match run_init(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(code) => code,
        },
        Some("doctor") => run_doctor_cmd(&args[1..]),
        _ => {
            eprintln!("usage: kuro init [--defaults] [--force]");
            eprintln!("       kuro doctor [--json]");
            ExitCode::from(2)
        }
    }
}

fn run_init(args: &[String]) -> Result<(), ExitCode> {
    let mut defaults = false;
    let mut force = false;
    for arg in args {
        match arg.as_str() {
            "--defaults" => defaults = true,
            "--force" => force = true,
            "--help" | "-h" => {
                eprintln!("kuro init --defaults   copy profiles/example to the user profile");
                eprintln!("kuro init --force      overwrite existing profile files");
                return Ok(());
            }
            other => {
                eprintln!("unknown init argument: {other}");
                return Err(ExitCode::from(2));
            }
        }
    }
    if !defaults {
        eprintln!("kuro init requires --defaults");
        return Err(ExitCode::from(2));
    }
    match init_defaults(force) {
        Ok(result) => {
            eprintln!(
                "profile at {} (copied {}, skipped {})",
                result.profile_dir.display(),
                result.copied,
                result.skipped
            );
            Ok(())
        }
        Err(InitError::ExampleMissing(path)) => {
            eprintln!("init failed: example profile missing at {}", path.display());
            Err(ExitCode::from(1))
        }
        Err(InitError::Io(err)) => {
            eprintln!("init failed: {err}");
            Err(ExitCode::from(1))
        }
    }
}

fn run_doctor_cmd(args: &[String]) -> ExitCode {
    let json = args.iter().any(|a| a == "--json");
    for arg in args {
        if arg != "--json" && arg != "--help" && arg != "-h" {
            eprintln!("unknown doctor argument: {arg}");
            return ExitCode::from(2);
        }
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("kuro doctor          run environment checks");
        eprintln!("kuro doctor --json   machine-readable report");
        return ExitCode::SUCCESS;
    }
    let (data_root, settings) = default_context();
    let repo_root = init::discover_repo_root();
    let rustc = rustc_version_line();
    let ctx = DoctorContext {
        data_root: &data_root,
        repo_root: &repo_root,
        settings: &settings,
        rustc_version: rustc.as_deref(),
    };
    let report = run_doctor(&ctx);
    if json {
        println!("{}", doctor_json(&ctx));
    } else {
        print_human(&report);
    }
    ExitCode::from(report.exit_code() as u8)
}

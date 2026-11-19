mod repo_map;

pub fn run() -> i32 {
    if repo_map::run() != 0 {
        return 1;
    }
    0
}

pub fn run_check() -> i32 {
    repo_map::run_check()
}

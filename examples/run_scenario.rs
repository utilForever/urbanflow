#[path = "run_scenario/mod.rs"]
pub mod runner;

fn main() -> std::process::ExitCode {
    match runner::run(std::env::args_os().skip(1)) {
        Ok(message) => {
            println!("{message}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

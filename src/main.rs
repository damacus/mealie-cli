use std::{env, process::ExitCode};

fn main() -> ExitCode {
    match mealie_cli::run_from_env() {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            let machine_readable =
                env::args_os().any(|argument| argument == "--json" || argument == "--ndjson");
            if machine_readable {
                eprintln!("{}", error.to_json_line());
            } else {
                eprintln!("{}", error.to_human());
            }
            ExitCode::from(error.exit_code())
        }
    }
}

use std::process::ExitCode;

fn main() -> ExitCode {
    match mealie_cli::run_from_env() {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", error.to_json_line());
            ExitCode::FAILURE
        }
    }
}

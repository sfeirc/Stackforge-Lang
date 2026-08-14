use std::env;
use std::fs;
use std::process::ExitCode;

fn print_usage() {
    eprintln!("usage: stackforge [--ast] <script.sf>");
    eprintln!();
    eprintln!("  --ast    run with the tree-walking interpreter instead of the bytecode VM");
    eprintln!("  --vm     run with the bytecode VM (default)");
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut ast_mode = false;
    let mut path: Option<String> = None;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--ast" => ast_mode = true,
            "--vm" => ast_mode = false,
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => path = Some(other.to_string()),
        }
    }

    let Some(path) = path else {
        print_usage();
        return ExitCode::FAILURE;
    };

    let src = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read '{}': {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let result = if ast_mode {
        stackforge::run_ast(&src, &mut lock)
    } else {
        stackforge::run_vm(&src, &mut lock)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

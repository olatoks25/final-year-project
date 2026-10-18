use std::{env, fs, process};

use tocin_rtos::{Simulator, parse_program};

const DEFAULT_TICKS: u64 = 200;

fn usage() {
    eprintln!(
        "Usage:\n  cargo run -- build <file.tocin>\n  cargo run -- run <file.tocin> [--ticks N]\n  cargo run -- test"
    );
}

fn parse_file(path: &str) -> Result<tocin_rtos::Program, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
    parse_program(&content).map_err(|errors| {
        errors
            .into_iter()
            .map(|d| format!("{}:{}: {}", d.line, d.column, d.message))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn command_build(path: &str) -> Result<(), String> {
    let program = parse_file(path)?;
    Simulator::new(program).map_err(|e| format!("runtime validation failed: {e}"))?;
    println!("build OK: {path}");
    Ok(())
}

fn command_run(path: &str, ticks: u64) -> Result<(), String> {
    let program = parse_file(path)?;
    let mut sim = Simulator::new(program).map_err(|e| format!("runtime validation failed: {e}"))?;
    let result = sim.run(ticks);
    for line in result.trace {
        println!("{line}");
    }
    println!(
        "run complete: finished={}, ticks={} (budget={ticks})",
        result.finished, result.ticks_executed
    );
    Ok(())
}

fn command_test() -> Result<(), String> {
    let smoke = r#"
channel q capacity 1;
task producer priority 2 { send q "hello"; yield; print "producer done"; }
task consumer priority 1 { recv q as msg; print msg; }
run producer, consumer;
"#;
    let program =
        parse_program(smoke).map_err(|e| format!("parser failed in smoke test: {e:?}"))?;
    let mut sim = Simulator::new(program).map_err(|e| format!("runtime setup failed: {e}"))?;
    let result = sim.run(50);
    if !result.finished {
        return Err("smoke test did not finish".to_string());
    }
    if !result.trace.iter().any(|l| l.contains("print hello")) {
        return Err("smoke test missing expected output".to_string());
    }
    println!("internal smoke test OK");
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
        process::exit(2);
    }

    let status = match args[1].as_str() {
        "build" if args.len() == 3 => command_build(&args[2]),
        "run" if args.len() >= 3 => {
            let mut ticks = DEFAULT_TICKS;
            if args.len() == 5 && args[3] == "--ticks" {
                ticks = args[4]
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --ticks value: {e}"))
                    .unwrap_or_else(|e| {
                        eprintln!("{e}");
                        process::exit(2);
                    });
            } else if args.len() > 3 {
                usage();
                process::exit(2);
            }
            command_run(&args[2], ticks)
        }
        "test" if args.len() == 2 => command_test(),
        _ => {
            usage();
            process::exit(2);
        }
    };

    if let Err(err) = status {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

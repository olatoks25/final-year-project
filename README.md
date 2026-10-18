# final-year-project

A self-contained **host-side educational RTOS simulator** with a small **TocinLang-inspired DSL**.

> ⚠️ The original referenced public implementation (`tafolabi009/tocin`) was unavailable (404) during research, so this repository provides an independent MVP that is buildable and testable now.

## Scope and limitations

This project is intentionally a deterministic Linux-host simulation for rapid iteration.

- ✅ Implements task lifecycle, priority scheduling, equal-priority round-robin fairness, yield/sleep, bounded channels, timer ticks, and idle tracing.
- ✅ Includes a minimal TocinLang-inspired frontend with parser diagnostics and source locations.
- ✅ Includes CLI commands and automated tests/CI.
- ❌ Not a bare-metal RTOS, no hardware interrupts, MMU/MPU setup, drivers, or real context switching.

## Stack

- Rust (stable), standard library only
- GitHub Actions on `ubuntu-latest`

## Prerequisites

- Rust toolchain (stable)

## Build, run, and test

From `/home/runner/work/final-year-project/final-year-project`:

```bash
cargo fmt --all
cargo build --locked
cargo test --locked

# Validate/compile a .tocin source file
cargo run --locked -- build examples/demo.tocin

# Run the simulator with trace output
cargo run --locked -- run examples/demo.tocin --ticks 200

# Run CLI internal smoke test
cargo run --locked -- test
```

## TocinLang-inspired DSL syntax (MVP)

```tocin
channel telemetry capacity 2;

task producer priority 3 {
  print "producer start";
  send telemetry "sensor:42";
  yield;
  sleep 2;
}

task consumer priority 2 {
  recv telemetry as msg;
  print msg;
}

run producer, consumer;
```

### Statements

- `print <expr>;` where `<expr>` is a string literal or variable
- `yield;`
- `sleep <ticks>;`
- `send <channel> <expr>;`
- `recv <channel> as <variable>;`

Parser diagnostics include line/column locations.

## RTOS simulator architecture

- **Frontend (`src/dsl.rs`)**: lexer + parser for channels, task declarations, and statements.
- **Kernel/runtime (`src/runtime.rs`)**:
  - task states: `Ready`, `Running`, `Blocked`, `Sleeping`, `Finished`
  - priority-based scheduler (higher numeric priority first)
  - FIFO ready queues for round-robin among equal priorities
  - bounded channels (`capacity`) with blocking send/recv
  - timer ticks and explicit idle tracing
- **CLI (`src/main.rs`)**:
  - `build <file>`: parse + runtime validation
  - `run <file> [--ticks N]`: execute and print deterministic trace
  - `test`: run an internal smoke scenario

## Reproducibility notes

- Deterministic tick-driven execution
- No randomness
- Stable task iteration and queue ordering

## Transition roadmap to a real target

1. Boot path: linker script, startup code, memory map, panic/abort strategy.
2. Context switching: architecture-specific save/restore + preemption ISR.
3. Time source: hardware timer interrupt replacing host tick loop.
4. Interrupt model: deferred work, critical sections, interrupt-safe queues.
5. Memory protection: MPU/MMU domains and task isolation.
6. Drivers: UART, timer, GPIO, storage/network as needed.
7. Verification: hardware-in-the-loop tests and timing/latency characterization.

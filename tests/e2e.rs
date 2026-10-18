use tocin_rtos::{Simulator, parse_program};

#[test]
fn sample_program_runs_to_completion() {
    let src = include_str!("../examples/demo.tocin");
    let program = parse_program(src).expect("sample parses");
    let mut sim = Simulator::new(program).expect("runtime setup");
    let result = sim.run(200);
    assert!(result.finished, "sample should finish");
    assert!(result.trace.iter().any(|l| l.contains("print sensor:42")));
    assert!(result.trace.iter().any(|l| l.contains("print sensor:43")));
}

#[test]
fn parser_diagnostic_on_invalid_syntax() {
    let src = "task x priority 1 { send c \"a\" } run x;";
    let errors = parse_program(src).expect_err("must fail");
    assert!(errors[0].message.contains("expected ';'"));
    assert!(errors[0].line >= 1);
    assert!(errors[0].column >= 1);
}

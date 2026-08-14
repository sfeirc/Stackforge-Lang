//! Integration tests: whole programs in `examples/*.sf`, run through both
//! execution back ends, checked against an *exact* expected output string
//! (not just "did not crash"). Each of these mirrors one of the mandatory
//! scenarios: recursive fibonacci, an accumulator loop, mutually recursive
//! functions, a reported syntax error with a line number, and a handled
//! runtime error (division by zero).

use stackforge::{run_ast, run_vm};

fn assert_both_backends_output(src: &str, expected: &str) {
    let mut ast_out = Vec::new();
    run_ast(src, &mut ast_out).expect("ast interpreter should succeed");
    assert_eq!(
        String::from_utf8(ast_out).unwrap(),
        expected,
        "ast interpreter output mismatch"
    );

    let mut vm_out = Vec::new();
    run_vm(src, &mut vm_out).expect("bytecode vm should succeed");
    assert_eq!(
        String::from_utf8(vm_out).unwrap(),
        expected,
        "bytecode vm output mismatch"
    );
}

#[test]
fn recursive_fibonacci_prints_first_ten_terms() {
    let src = include_str!("../examples/fibonacci.sf");
    assert_both_backends_output(src, "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n");
}

#[test]
fn recursive_factorial_of_ten() {
    let src = include_str!("../examples/factorial.sf");
    assert_both_backends_output(src, "3628800\n");
}

#[test]
fn accumulator_loop_sums_one_to_one_hundred() {
    let src = include_str!("../examples/accumulator.sf");
    assert_both_backends_output(src, "5050\n");
}

#[test]
fn mutually_recursive_functions_agree_on_parity() {
    let src = include_str!("../examples/mutual_recursion.sf");
    assert_both_backends_output(src, "true\nfalse\nfalse\ntrue\n");
}

#[test]
fn arrays_and_maps_program_produces_expected_values() {
    let src = include_str!("../examples/arrays_and_maps.sf");
    assert_both_backends_output(src, "[0, 1, 4, 9, 16]\nstackforge 1\n30\n");
}

#[test]
fn full_demo_program_runs_end_to_end_on_both_backends() {
    let src = include_str!("../examples/demo.sf");
    let expected = concat!(
        "=== fibonacci(0..9) ===\n",
        "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n",
        "=== factorial(10) ===\n",
        "3628800\n",
        "=== accumulator: sum 1..100 ===\n",
        "5050\n",
        "=== mutual recursion: is_even/is_odd ===\n",
        "true false false true\n",
        "=== arrays and maps ===\n",
        "[0, 1, 4, 9, 16]\n",
        "stackforge 1\n",
        "=== done ===\n",
    );
    assert_both_backends_output(src, expected);
}

#[test]
fn syntax_error_is_reported_with_correct_line_and_not_a_panic() {
    let src = include_str!("../examples/error_syntax.sf");
    let mut out = Vec::new();
    let err = run_vm(src, &mut out).expect_err("malformed program must not succeed");
    let msg = err.to_string();
    assert!(
        msg.contains("line 7"),
        "expected the error to point at line 7, got: {}",
        msg
    );
    assert_eq!(err.phase(), "syntax error");

    // The AST interpreter shares the same parser, so it must fail identically.
    let mut out2 = Vec::new();
    let err2 = run_ast(src, &mut out2).expect_err("malformed program must not succeed on ast backend either");
    assert_eq!(err.to_string(), err2.to_string());
}

#[test]
fn division_by_zero_is_a_handled_runtime_error_on_both_backends() {
    let src = include_str!("../examples/error_runtime.sf");

    let mut vm_out = Vec::new();
    let vm_err = run_vm(src, &mut vm_out).expect_err("division by zero must not succeed");
    assert_eq!(vm_err.phase(), "runtime error");
    assert_eq!(vm_err.line(), 6);
    assert!(vm_err.to_string().contains("division by zero"));
    // The first call (divide(10, 2)) must have printed before the second
    // call blew up -- proves the error is caught mid-program, not that the
    // whole thing silently produced nothing.
    assert_eq!(String::from_utf8(vm_out).unwrap(), "5\n");

    let mut ast_out = Vec::new();
    let ast_err = run_ast(src, &mut ast_out).expect_err("division by zero must not succeed");
    assert_eq!(ast_err.phase(), "runtime error");
    assert_eq!(ast_err.line(), 6);
    assert_eq!(String::from_utf8(ast_out).unwrap(), "5\n");
}

#[test]
fn undefined_variable_is_a_compile_time_error_for_the_vm() {
    let src = "print totally_undefined_name;";
    let mut out = Vec::new();
    let err = run_vm(src, &mut out).unwrap_err();
    assert_eq!(err.phase(), "compile error");
}

#[test]
fn array_index_out_of_bounds_is_a_handled_runtime_error() {
    let src = "let a = [1, 2, 3];\nlet x = a[10];\n";
    let mut out = Vec::new();
    let err = run_vm(src, &mut out).unwrap_err();
    assert_eq!(err.phase(), "runtime error");
    assert_eq!(err.line(), 2);
}

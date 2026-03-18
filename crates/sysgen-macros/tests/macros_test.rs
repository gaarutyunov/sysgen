#[test]
fn implements_compiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/cases/implements_basic.rs");
    t.pass("tests/cases/implements_multi.rs");
    t.pass("tests/cases/verifies_test_fn.rs");
    t.compile_fail("tests/cases/implements_invalid_attr.rs");
}

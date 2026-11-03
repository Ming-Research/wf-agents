// A probe of the fast CI (#269): a test unit that does not compile. Reverted in the next commit.

#[test]
fn a_probe_that_does_not_compile() {
    let n: u32 = "not a number";
    assert_eq!(n, 0);
}

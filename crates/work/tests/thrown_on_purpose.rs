// Thrown on purpose, to see what the job `check` shows of a failing test and of a format error. To be reverted.
#[test]
fn fails_on_purpose_to_show_the_red_path() {
    let  seen=1;
    assert_eq!( seen,2 , "thrown on purpose: the red path of the job check" );
}

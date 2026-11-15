//! The form of return the agents' program is given: the one the flow and what is written beside it make
//! (`flow::form`), the same bytes on every call of every cell of the flow.

use service::Act;

use crate::{Fake, Real, ended};

#[tokio::test]
async fn every_call_of_every_cell_is_given_the_form_of_its_flow_as_the_files_beside_it_make_it() {
    let mut real = Real::trial(Fake::default());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3, "the plan, the build and the review");
    let form = real.form();
    assert!(form.contains("\"query\""), "the build may ask for `query`: {form}");
    for n in 1..=calls.len() {
        assert_eq!(real.form_given(n), form, "call {n}");
    }
}

//! What a round's usage record keeps of the model's time, from the result line the CLI prints at the end of a run.

use sessions::cli::{Reply, read_stream};

// The model time the CLI reports goes into the usage record as it reports it, in milliseconds.
#[test]
fn the_usage_record_keeps_the_model_time_the_cli_reports() {
    let text = [
        r#"{"type":"system","subtype":"init","session_id":"s1","model":"m"}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1","total_cost_usd":0.5,"duration_ms":60000,"duration_api_ms":41000,"num_turns":2,"usage":{"input_tokens":3}}"#,
    ]
    .join("\n");
    let mut r = Reply::default();
    read_stream(&text, &mut r);
    assert_eq!(r.usage["duration_api_ms"], 41_000);
}

// A run whose result gives no model time has no such key: neither null nor 0. The result itself was read, so the
// absence is the run's, not a result that was missed.
#[test]
fn a_run_that_reports_no_model_time_has_no_such_key() {
    let text = [
        r#"{"type":"system","subtype":"init","session_id":"s1","model":"m"}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1","total_cost_usd":0.5,"duration_ms":60000,"num_turns":2,"usage":{"input_tokens":3}}"#,
    ]
    .join("\n");
    let mut r = Reply::default();
    read_stream(&text, &mut r);
    assert_eq!(r.usage["session_cost_usd"], 0.5);
    assert!(r.usage.get("duration_api_ms").is_none());
}

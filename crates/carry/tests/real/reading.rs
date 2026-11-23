//! What the engine says of a result the command line printed that holds no return, in the words beside this
//! repository's flows (`flow/agents/cost.toml`): the sentences it makes are read whole, whatever the program said.

use std::path::Path;

use carry::cli::read;
use serde_json::{Value, json};

/// A partial message of the agent's own, as the command line writes it.
fn event(event: Value) -> String {
    json!({ "type": "stream_event", "event": event, "parent_tool_use_id": null }).to_string()
}

/// The lines of a call that is an attempt at the return whose input is cut short, so that it is no JSON, which the
/// command line refused in these words.
fn broken_attempt(refused: &str) -> Vec<String> {
    let id = "so-1";
    let block = json!({ "type": "tool_use", "id": id, "name": "StructuredOutput", "input": {} });
    let delta = json!({ "type": "input_json_delta", "partial_json": r#"{"outcome": "# });
    let message = json!({ "role": "assistant", "content": [block.clone()] });
    let answer = json!({ "role": "user", "content": [
        json!({ "type": "tool_result", "content": refused, "is_error": true, "tool_use_id": id })
    ] });
    vec![
        json!({ "type": "system", "subtype": "init", "session_id": "s" }).to_string(),
        event(json!({ "type": "message_start", "message": {} })),
        event(json!({ "type": "content_block_start", "index": 0, "content_block": block })),
        event(json!({ "type": "content_block_delta", "index": 0, "delta": delta })),
        json!({ "type": "assistant", "message": message, "parent_tool_use_id": null }).to_string(),
        event(json!({ "type": "content_block_stop", "index": 0 })),
        json!({ "type": "user", "message": answer, "parent_tool_use_id": null }).to_string(),
        event(json!({ "type": "message_stop" })),
    ]
}

/// The result of a call the command line gave up on the return of, saying this.
fn gave_up(said: &str) -> String {
    json!({ "type": "result", "subtype": "error_max_structured_output_retries", "is_error": true, "result": null,
        "errors": [said], "total_cost_usd": 0.4, "usage": { "input_tokens": 10, "output_tokens": 300 } })
    .to_string()
}

// Whatever the command line said as it gave up, with no full stop at its end or with one, the reason made of it and of
// what is wrong with the last attempt is two sentences, not one run on: the sentence of the words `last_attempt` that
// follows what the program said begins a sentence. Reference: the item's want that sentences made of the files'
// words and of others' words are read whole; English sentence punctuation.
#[test]
fn the_reason_a_return_gave_up_on_is_read_as_sentences_whether_or_not_the_program_ended_what_it_said_with_a_stop() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let program = flow::agents::read(&root).expect("the files are read").cost.program;
    let refused = "Output does not match required schema";
    for ending in ["", ".", "?", "!"] {
        let said = format!(
            "Failed to provide valid structured output after 3 attempts — last StructuredOutput error: it cannot be \
             parsed{ending}"
        );
        let mut lines = broken_attempt(refused);
        lines.push(gave_up(&said));
        let no = read(lines.join("\n").as_bytes(), &program).expect_err("it is no return");
        let (before, after) = no
            .why
            .split_once(" Its last attempt at the return is no JSON")
            .unwrap_or_else(|| panic!("the reason says the attempt is no JSON: {}", no.why));
        assert!(
            before.ends_with(['.', '?', '!']),
            "what the program said ({ending:?}) runs into the next sentence: {}",
            no.why
        );
        assert!(after.contains(refused), "{}", no.why);
    }
}

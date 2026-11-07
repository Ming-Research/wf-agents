// A check for one change only, not kept: the hash of what the compiled flow gives agents and the page, the field titles
// and the names of inputs no agent knows left out, and of the formats agents are sent.

use serde_json::Value;

fn context() -> flow::Context {
    flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    }
}

#[test]
fn what_agents_are_given_hashes_as_before() {
    let c = flow::compile(flow::FLOW, &context()).unwrap_or_else(|e| panic!("{e}"));
    let mut v = serde_json::to_value(&c).unwrap();
    for g in v["graphs"].as_array_mut().unwrap() {
        for cell in g["cells"].as_array_mut().unwrap() {
            cell.as_object_mut().unwrap().remove("titles");
            for x in cell["inputs"].as_array_mut().unwrap() {
                if x.get("common") != Some(&Value::Bool(true)) {
                    x.as_object_mut().unwrap().remove("name");
                }
            }
        }
    }
    let mut texts = Vec::new();
    for g in &c.graphs {
        texts.push(flow::formats::common_format(g));
        for cell in &g.cells {
            texts.push(flow::formats::cell_format(g, cell));
            texts.push(flow::formats::cell_inputs(cell));
        }
    }
    let compiled = flow::formats::hash(&v);
    let formats = flow::formats::hash(&serde_json::json!(texts));
    // As main compiled its flow at 73b1b383, before the titles and the names were taken out.
    assert_eq!(
        format!("{compiled} {formats}"),
        "c72caaaea15ff1934bc4017e83e721835683b497c588ce1d344d536e624f8160 \
         653b9c6b1768e54f2a7404106d1d00cb3931ec7c202c9456504ad38a33906803",
        "the hashes"
    );
}

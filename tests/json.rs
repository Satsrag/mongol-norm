//! Unit test of the harness's dependency-free JSON reader (`tests/common/json.rs`), kept in its
//! own binary so it runs once rather than inside every integration test.

mod common;

use common::json::Json;

#[test]
fn reads_the_shapes_the_goldens_use() {
    let value = Json::parse(
        r#"{"id":"x","cps":[6176, 6155],"shape":["A","Aa"],"n":null,"t":true,"e":{},"s":"ᠠ\n"}"#,
    );
    assert_eq!(value.index("id").as_str(), "x");
    assert_eq!(value.index("cps").as_array()[1].as_u64(), 6155);
    assert_eq!(value.index("shape").strings(), vec!["A", "Aa"]);
    assert_eq!(value.index("n").as_str_opt(), None);
    assert_eq!(value.index("t"), &Json::Bool(true));
    assert_eq!(value.index("e"), &Json::Object(vec![]));
    assert_eq!(value.index("s").as_str(), "\u{1820}\n");
}

use playgraph_core::codegen::CodegenError;
use playgraph_core::model::{
    Cache, Edge, Eval, GraphNode, OutputDef, ParamDef, Playground, StageDef,
};
use playgraph_core::{generate, parse_signature, parse_stage};

fn stage(name: &str, inputs: &[(&str, &str)], outputs: &[(&str, &str)], body: &str) -> StageDef {
    StageDef {
        name: name.to_string(),
        generics: String::new(),
        where_clause: String::new(),
        is_async: false,
        inputs: inputs
            .iter()
            .map(|(name, ty)| ParamDef {
                name: name.to_string(),
                type_name: ty.to_string(),
            })
            .collect(),
        outputs: outputs
            .iter()
            .map(|(name, ty)| OutputDef {
                name: name.to_string(),
                type_name: ty.to_string(),
            })
            .collect(),
        body: body.to_string(),
        eval: Eval::Urgent,
        cache: Cache::None,
        state_type: None,
    }
}

fn node(id: &str, stage: &str) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        stage: stage.to_string(),
        type_args: Vec::new(),
        x: 0.0,
        y: 0.0,
    }
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
    Edge {
        from_node: from.to_string(),
        from_port: from_port.to_string(),
        to_node: to.to_string(),
        to_port: to_port.to_string(),
    }
}

#[test]
fn parses_inputs_and_output() {
    let signature = parse_stage("fn add(a: i32, b: &str) -> i32 { let _ = b; a }").unwrap();
    assert_eq!(signature.fn_name, "add");
    assert_eq!(signature.inputs.len(), 2);
    assert_eq!(signature.inputs[0].name, "a");
    assert_eq!(signature.inputs[0].type_name, "i32");
    assert_eq!(signature.inputs[1].type_name, "str");
    assert_eq!(signature.outputs[0].type_name, "i32");
}

#[test]
fn parses_bare_signature() {
    let signature = parse_signature("fn Double(value: i32) -> i32").unwrap();
    assert_eq!(signature.fn_name, "Double");
    assert_eq!(signature.inputs[0].type_name, "i32");
}

#[test]
fn generates_metadata_attribute() {
    let mut producer = stage("Producer", &[], &[("out", "i32")], "40");
    producer.eval = Eval::Lazy;
    producer.cache = Cache::Last;
    producer.state_type = Some("u32".to_string());

    let consumer = stage(
        "Consumer",
        &[("value", "i32")],
        &[("out", "i32")],
        "value + 2",
    );

    let playground = Playground {
        prelude: String::new(),
        stages: vec![producer, consumer],
        nodes: vec![node("a", "Producer"), node("b", "Consumer")],
        edges: vec![edge("a", "out", "b", "value")],
    };

    let generated = generate(&playground).unwrap();
    let compact = generated.replace([' ', '\n'], "");
    assert!(compact.contains("#[stage(lazy,cache_last,out(out:i32),state(u32))]"));
    assert!(generated.contains("registry.register::<Producer>()"));
    assert!(compact.contains("node_0:out=>node_1:value"));
    assert!(generated.contains("execute_async"));
    assert!(!generated.contains("println!"));
}

#[test]
fn generates_multi_output() {
    let playground = Playground {
        prelude: String::new(),
        stages: vec![
            stage(
                "Split",
                &[],
                &[("number", "i32"), ("text", "String")],
                "(42, String::from(\"hi\"))",
            ),
            stage("ConsumeNumber", &[("number", "i32")], &[("out", "i32")], ""),
            stage(
                "ConsumeText",
                &[("text", "String")],
                &[("out", "String")],
                "",
            ),
        ],
        nodes: vec![
            node("p", "Split"),
            node("n", "ConsumeNumber"),
            node("t", "ConsumeText"),
        ],
        edges: vec![
            edge("p", "number", "n", "number"),
            edge("p", "text", "t", "text"),
        ],
    };
    let generated = generate(&playground).unwrap();
    let compact = generated.replace([' ', '\n'], "");
    assert!(compact.contains("#[stage(out(number:i32,text:String))]"));
    assert!(generated.contains("execute_async"));
}

#[test]
fn generates_generic_node() {
    let identity = StageDef {
        name: "Identity".to_string(),
        generics: "T".to_string(),
        where_clause: "T: std::fmt::Debug".to_string(),
        is_async: false,
        inputs: vec![ParamDef {
            name: "value".to_string(),
            type_name: "T".to_string(),
        }],
        outputs: vec![OutputDef {
            name: "out".to_string(),
            type_name: "T".to_string(),
        }],
        body: "value".to_string(),
        eval: Eval::Urgent,
        cache: Cache::None,
        state_type: None,
    };
    let mut identity_node = node("id", "Identity");
    identity_node.type_args = vec!["i32".to_string()];

    let playground = Playground {
        prelude: String::new(),
        stages: vec![
            identity,
            stage("Source", &[], &[("out", "i32")], "7"),
            stage("Sink", &[("value", "i32")], &[], ""),
        ],
        nodes: vec![node("src", "Source"), identity_node, node("sink", "Sink")],
        edges: vec![
            edge("src", "out", "id", "value"),
            edge("id", "out", "sink", "value"),
        ],
    };

    let generated = generate(&playground).unwrap();
    let compact = generated.replace([' ', '\n'], "");
    assert!(compact.contains("registry.register::<Identity<i32>>()"));
    assert!(compact.contains("whereT:std::fmt::Debug"));
    assert!(generated.contains("execute_async"));
}

#[test]
fn allows_inference_across_generic_edges() {
    let default_value = StageDef {
        name: "DefaultValue".to_string(),
        generics: "T".to_string(),
        where_clause: "T: std::default::Default".to_string(),
        outputs: vec![OutputDef {
            name: "out".to_string(),
            type_name: "T".to_string(),
        }],
        body: "T::default()".to_string(),
        ..StageDef::default()
    };
    let add = StageDef {
        name: "Add".to_string(),
        generics: "T".to_string(),
        where_clause: "T: num_traits::Num".to_string(),
        inputs: vec![
            ParamDef {
                name: "a".to_string(),
                type_name: "T".to_string(),
            },
            ParamDef {
                name: "b".to_string(),
                type_name: "T".to_string(),
            },
        ],
        outputs: vec![OutputDef {
            name: "out".to_string(),
            type_name: "T".to_string(),
        }],
        body: "a + b".to_string(),
        ..StageDef::default()
    };
    // Pin the source to i32 but leave `Add` generic; the compiler infers `T`.
    let mut source = node("dv", "DefaultValue");
    source.type_args = vec!["i32".to_string()];

    let playground = Playground {
        prelude: String::new(),
        stages: vec![default_value, add],
        nodes: vec![source, node("add", "Add")],
        edges: vec![edge("dv", "out", "add", "a")],
    };
    assert!(generate(&playground).is_ok());
}

#[test]
fn rejects_type_mismatch() {
    let playground = Playground {
        prelude: String::new(),
        stages: vec![
            stage("Producer", &[], &[("out", "i32")], "1"),
            stage("Consumer", &[("value", "String")], &[], ""),
        ],
        nodes: vec![node("a", "Producer"), node("b", "Consumer")],
        edges: vec![edge("a", "out", "b", "value")],
    };
    assert!(matches!(
        generate(&playground),
        Err(CodegenError::TypeMismatch { .. })
    ));
}

#[test]
fn rejects_cycles() {
    let playground = Playground {
        prelude: String::new(),
        stages: vec![stage(
            "Step",
            &[("value", "i32")],
            &[("out", "i32")],
            "value",
        )],
        nodes: vec![node("a", "Step"), node("b", "Step")],
        edges: vec![
            edge("a", "out", "b", "value"),
            edge("b", "out", "a", "value"),
        ],
    };
    assert!(matches!(generate(&playground), Err(CodegenError::Cycle(_))));
}

#[test]
fn emits_prelude_verbatim_before_stages() {
    let playground = Playground {
        prelude: "// my helper\nfn helper(x: i32) -> i32 { x + 1 }".to_string(),
        stages: vec![stage("Source", &[], &[("out", "i32")], "helper(1)")],
        nodes: vec![node("a", "Source")],
        edges: vec![],
    };
    let generated = generate(&playground).unwrap();
    // Comments survive (the prelude is not re-parsed).
    assert!(generated.contains("// my helper"));
    assert!(generated.contains("fn helper(x: i32) -> i32 { x + 1 }"));
    let prelude_at = generated.find("fn helper").unwrap();
    let stage_at = generated.find("fn Source").unwrap();
    assert!(prelude_at < stage_at, "prelude should precede the stages");
}

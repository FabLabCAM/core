use super::{connection, context, plan, response, server};
use fablabcam_studio::{
    assistant::{actions, context::ContextMeta, provider, ContextOptions, ContextSnapshot, Mode, Plan, Provider, Workspace},
    furniture,
};
use serde_json::{json, Value};
use std::sync::{atomic::AtomicBool, Arc};

fn workspace() -> (Workspace, ContextSnapshot, String) {
    let w =
        Workspace { document_id: "division-test".into(), revision: 0, project: Default::default(), script: String::new(), furniture: None };
    let context = ContextSnapshot::capture(&w, &ContextMeta::default(), &ContextOptions::default()).unwrap();
    let w = actions::prepare(&w, &context, serde_json::from_value(plan()).unwrap(), Mode::Agent, Arc::new(AtomicBool::new(false)))
        .unwrap()
        .after;
    let context = ContextSnapshot::capture(&w, &ContextMeta::default(), &ContextOptions { furniture: true, ..Default::default() }).unwrap();
    let id = context.data["furniture_structure"]["cells"].as_array().unwrap().iter().find(|c| c["leaf"] == true).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    (w, context, id)
}
fn wire(arguments: Value) -> String {
    format!(
        "{}\n{}\n",
        json!({"model":"test-model","message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"divide_cell","arguments":arguments}}]},"done":false}),
        json!({"model":"test-model","message":{"content":""},"done":true,"done_reason":"stop"})
    )
}

#[tokio::test]
async fn native_division_remains_a_reviewed_plan_and_preserves_tool_continuation() {
    let (w, context, id) = workspace();
    let before = w.fingerprint();
    let server = server(
        Provider::Ollama,
        vec![(200, wire(json!({"cell_id":id,"layout":"side_by_side","count":2}))), (200, response(Provider::Ollama, false))],
    );
    let c = connection(Provider::Ollama, server.endpoint);
    let request = provider::request(&c, Mode::Agent, &context, &[], "Diviser la case").unwrap();
    let reply = provider::chat(&c, "", request, Mode::Agent, |_| {}).await.unwrap();
    let prepared = actions::prepare(&w, &context, reply.plan.clone().unwrap(), Mode::Agent, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(w.fingerprint(), before);
    assert_eq!(furniture::structure(prepared.after.furniture.as_ref().unwrap().dimensions()).unwrap().panels.len(), 8);
    assert!(actions::authorize(&prepared, &w, &context, Mode::Plan, false).is_err());
    assert!(actions::authorize(&prepared, &w, &context, Mode::Agent, true).is_err());
    let mut changed = w.clone();
    changed.revision += 1;
    assert!(actions::authorize(&prepared, &changed, &context, Mode::Agent, false).is_err());
    actions::authorize(&prepared, &w, &context, Mode::Agent, false).unwrap();
    let done =
        provider::chat(&c, "", reply.continuation("Lot appliqué après validation humaine").unwrap(), Mode::Agent, |_| {}).await.unwrap();
    assert!(done.plan.is_none());
    let sent = server.requests.recv().unwrap();
    let tool = sent.body["tools"].as_array().unwrap().iter().find(|t| t["function"]["name"] == "divide_cell").unwrap();
    assert!(tool["function"]["parameters"]["properties"]["cell_id"]["enum"].as_array().unwrap().contains(&json!(id)));
    assert_eq!(tool["function"]["parameters"]["properties"]["layout"]["enum"], json!(["side_by_side", "stacked"]));
    assert!(!tool["function"]["parameters"].to_string().contains("$ref"));
    let next = server.requests.recv().unwrap();
    assert!(next.body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["role"] == "tool" && m["content"] == "Lot appliqué après validation humaine"));
    assert!(next.body.to_string().contains("divide_cell"));
}

#[tokio::test]
async fn native_division_rejects_invalid_arguments_and_preserves_domain_checks() {
    let (w, context, id) = workspace();
    for arguments in [
        json!({"cell_id":id,"layout":"side_by_side"}),
        json!({"cell_id":id,"layout":true,"count":2}),
        json!({"cell_id":id,"layout":"side_by_side","count":2,"code":"delete(shapes());"}),
    ] {
        let server = server(Provider::Ollama, vec![(200, wire(arguments))]);
        let c = connection(Provider::Ollama, server.endpoint);
        let request = provider::request(&c, Mode::Agent, &context, &[], "Diviser").unwrap();
        assert!(provider::chat(&c, "", request, Mode::Agent, |_| {}).await.is_err());
    }
    for (cell, count) in [(id.as_str(), 0), ("invented", 2), ("root", 2)] {
        let server = server(Provider::Ollama, vec![(200, wire(json!({"cell_id":cell,"layout":"side_by_side","count":count})))]);
        let c = connection(Provider::Ollama, server.endpoint);
        let request = provider::request(&c, Mode::Agent, &context, &[], "Diviser").unwrap();
        let p: Plan = provider::chat(&c, "", request, Mode::Agent, |_| {}).await.unwrap().plan.unwrap();
        assert!(actions::prepare(&w, &context, p, Mode::Agent, Arc::new(AtomicBool::new(false))).is_err());
    }
}

#[tokio::test]
async fn division_is_not_exposed_without_furniture_or_in_ask_or_to_other_providers() {
    let (_, furniture_context, id) = workspace();
    for p in Provider::ALL {
        for (mode, context, tools) in
            [(Mode::Ask, furniture_context.clone(), true), (Mode::Plan, context(), true), (Mode::Plan, furniture_context.clone(), false)]
        {
            let c = fablabcam_studio::assistant::Connection { tools, ..connection(p, p.endpoint().into()) };
            let r = provider::request(&c, mode, &context, &[], "Diviser").unwrap();
            assert!(!r.tools.unwrap_or_default().iter().any(|t| t.name.as_str() == "divide_cell"));
        }
        if p != Provider::Ollama {
            let c = connection(p, p.endpoint().into());
            let r = provider::request(&c, Mode::Agent, &furniture_context, &[], "Diviser").unwrap();
            assert_eq!(r.tools.unwrap().len(), 1);
        }
    }
    for (mode, context) in [(Mode::Ask, furniture_context), (Mode::Agent, context())] {
        let server = server(Provider::Ollama, vec![(200, wire(json!({"cell_id":id,"layout":"side_by_side","count":2})))]);
        let c = connection(Provider::Ollama, server.endpoint);
        let r = provider::request(&c, mode, &context, &[], "Diviser").unwrap();
        assert!(provider::chat(&c, "", r, mode, |_| {}).await.is_err());
    }
}

#[tokio::test]
async fn text_that_looks_like_a_division_is_never_repaired_into_a_tool() {
    let (_, context, id) = workspace();
    let text = json!({"name":"divide_cell","arguments":{"cell_id":id,"layout":"side_by_side","count":2}}).to_string();
    let body = format!("{}\n", json!({"model":"test-model","message":{"content":text},"done":true,"done_reason":"stop"}));
    let server = server(Provider::Ollama, vec![(200, body)]);
    let c = connection(Provider::Ollama, server.endpoint);
    let r = provider::request(&c, Mode::Agent, &context, &[], "Diviser").unwrap();
    let reply = provider::chat(&c, "", r, Mode::Agent, |_| {}).await.unwrap();
    assert!(reply.plan.is_none());
    assert!(reply.call_id.is_none());
}

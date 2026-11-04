mod assistant_support;
use assistant_support::*;
use fablabcam_studio::assistant::{
    actions, provider,
    worker::{Event, Job},
    Mode, Provider,
};
use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[tokio::test]
async fn four_wire_protocols_stream_plan_and_roundtrip_tool_result() {
    for p in Provider::ALL {
        let server = server(p, vec![(200, response(p, true)), (200, response(p, false))]);
        let c = connection(p, server.endpoint);
        let context = context();
        let request = provider::request(&c, Mode::Agent, &context, &[], "Créer le caisson de test").unwrap();
        let mut streamed = String::new();
        let reply = provider::chat(&c, "synthetic-test-secret", request, Mode::Agent, |s| streamed += &s)
            .await
            .unwrap_or_else(|e| panic!("{p:?}: {e}"));
        assert_eq!(streamed, reply.text);
        assert!(!streamed.is_empty());
        let plan = reply.plan.clone().expect("typed plan");
        let w = fablabcam_studio::assistant::Workspace {
            document_id: "synthetic-test".into(),
            revision: 0,
            project: Default::default(),
            script: String::new(),
            furniture: None,
        };
        let prepared = actions::prepare(&w, &context, plan, Mode::Agent, Arc::new(AtomicBool::new(false))).unwrap();
        let doc = prepared.after.furniture.as_ref().unwrap();
        assert_eq!(doc.dimensions().width, 600.0);
        let request = reply.continuation("Caisson créé après validation humaine").unwrap();
        let done = provider::chat(&c, "synthetic-test-secret", request, Mode::Agent, |_| {})
            .await
            .unwrap_or_else(|e| panic!("roundtrip {p:?}: {e}"));
        assert!(done.plan.is_none());
        let first = server.requests.recv_timeout(Duration::from_secs(1)).unwrap();
        let second = server.requests.recv_timeout(Duration::from_secs(1)).unwrap();
        let expected = match p {
            Provider::OpenAi => "/responses",
            Provider::Claude => "/messages",
            Provider::Mistral => "/chat/completions",
            Provider::Ollama => "/api/chat",
        };
        assert_eq!(first.path, expected);
        assert_eq!(second.path, expected);
        assert_eq!(first.body["model"], "test-model");
        assert!(first.body["tools"].as_array().is_some_and(|t| !t.is_empty()));
        assert!(!first.body.to_string().contains("synthetic-test-secret"));
        if p != Provider::Ollama {
            assert!(first.headers.contains("synthetic-test-secret"), "{p:?}");
        }
        assert!(second.body.to_string().contains("Caisson créé après validation humaine"));
        match p {
            Provider::OpenAi => {
                assert_eq!(first.body["store"], false);
                assert!(second.body.to_string().contains("function_call_output"));
            }
            Provider::Claude => assert!(second.body.to_string().contains("tool_result")),
            _ => assert!(second.body.to_string().contains("\"role\":\"tool\"")),
        }
    }
}

#[tokio::test]
async fn ask_never_exposes_tools_and_rejects_unsolicited_calls() {
    for p in Provider::ALL {
        let server = server(p, vec![(200, response(p, true))]);
        let c = connection(p, server.endpoint);
        let r = provider::request(&c, Mode::Ask, &context(), &[], "Explique").unwrap();
        assert!(provider::chat(&c, "test", r, Mode::Ask, |_| {}).await.is_err());
        let sent = server.requests.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(sent.body.get("tools").is_none_or(|v| v.is_null() || v.as_array().is_some_and(|a| a.is_empty())));
    }
}

#[tokio::test]
async fn interrupted_invalid_unknown_or_truncated_plans_are_never_applicable() {
    for p in Provider::ALL {
        let valid = response(p, true);
        let broken = match p {
            Provider::OpenAi => valid.replace("\"status\":\"completed\"", "\"status\":\"incomplete\""),
            Provider::Claude => valid.replace("\"stop_reason\":\"tool_use\"", "\"stop_reason\":\"max_tokens\""),
            Provider::Mistral => valid.replace("\"finish_reason\":\"tool_calls\"", "\"finish_reason\":\"length\""),
            Provider::Ollama => valid.replace("\"done_reason\":\"stop\"", "\"done_reason\":\"length\""),
        };
        for body in [
            broken,
            valid.replace("submit_plan", "execute_shell"),
            valid.replace("create_cabinet", "unknown_action"),
            valid[..valid.len() / 3].to_string(),
        ] {
            let server = server(p, vec![(200, body)]);
            let c = connection(p, server.endpoint);
            let r = provider::request(&c, Mode::Agent, &context(), &[], "Créer").unwrap();
            assert!(provider::chat(&c, "test", r, Mode::Agent, |_| {}).await.is_err(), "{p:?}");
        }
    }
}

#[tokio::test]
async fn errors_are_classified_without_echoing_credentials_or_payloads() {
    for (status, expected) in [(401, "Clé invalide"), (429, "Quota"), (404, "introuvable"), (422, "Requête refusée")] {
        let server = server(Provider::Mistral, vec![(status, "{\"error\":\"synthetic-test-secret and private context\"}".into())]);
        let c = connection(Provider::Mistral, server.endpoint);
        let r = provider::request(&c, Mode::Ask, &context(), &[], "Explique").unwrap();
        let error = provider::chat(&c, "synthetic-test-secret", r, Mode::Ask, |_| {}).await.err().unwrap();
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("synthetic-test-secret"));
        assert!(!error.contains("private context"));
    }
}

#[test]
fn worker_stop_interrupts_waiting_network_and_does_not_publish_a_reply() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let c = connection(Provider::Ollama, format!("http://{}/", listener.local_addr().unwrap()));
    let r = provider::request(&c, Mode::Agent, &context(), &[], "Créer").unwrap();
    let mut job = Job::chat(1, c, String::new(), r, Mode::Agent, Arc::new(|| {}));
    job.cancel();
    assert!(matches!(job.events.recv_timeout(Duration::from_secs(2)).unwrap(), Event::Error(_)));
    assert!(job.cancelled.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn configuration_and_history_never_store_a_key() {
    use fablabcam_studio::assistant::{storage, Session};
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("history.json");
    let mut session = Session::default();
    session.push(true, storage::redact("test synthetic-test-secret", "synthetic-test-secret"), &context(), &session.connection());
    storage::save(&file, &session).unwrap();
    assert!(!std::fs::read_to_string(&file).unwrap().contains("synthetic-test-secret"));
    assert_eq!(storage::load(&file).unwrap().entries.len(), 1);
    let mut c = connection(Provider::OpenAi, "https://user:secret@example.org/v1/".into());
    assert!(c.check().is_err());
    c.endpoint = "http://example.org/v1/".into();
    assert!(c.check().is_err());
    c.endpoint = Provider::OpenAi.endpoint().into();
    c.tools = false;
    assert!(provider::request(&c, Mode::Agent, &context(), &[], "Créer").is_err());
    assert!(provider::request(&c, Mode::Plan, &context(), &[], "Préparer").is_ok());
}

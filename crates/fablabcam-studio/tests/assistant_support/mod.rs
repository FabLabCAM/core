use fablabcam_studio::assistant::{context::ContextMeta, Connection, ContextOptions, ContextSnapshot, Provider, Workspace};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};

pub fn context() -> ContextSnapshot {
    ContextSnapshot::capture(
        &Workspace {
            document_id: "synthetic-test".into(),
            revision: 0,
            project: Default::default(),
            script: String::new(),
            furniture: None,
        },
        &ContextMeta { page: "Meubles".into(), ..Default::default() },
        &ContextOptions::default(),
    )
    .unwrap()
}
pub fn plan() -> Value {
    json!({"title":"Caisson de test","assumptions":[],"missing":[],"steps":[{
        "explanation":"Créer un caisson 600 × 400 × 800 mm",
        "action":{"type":"create_cabinet","name":"Caisson test","width":600.0,"depth":400.0,
            "height":800.0,"thickness":18.0,"shelves":2,"back":true}}]})
}
fn sse(kind: &str, value: Value) -> String {
    format!("event: {kind}\ndata: {value}\n\n")
}

/// Synthetic wire fixtures: exercise the shipped adapters, not a replacement parser.
pub fn response(p: Provider, tool: bool) -> String {
    let args = plan().to_string();
    match p {
        Provider::OpenAi => {
            let mut body =
                sse("response.output_text.delta", json!({"type":"response.output_text.delta","delta":"Proposition vérifiable."}));
            if tool {
                body += &sse(
                    "response.output_item.added",
                    json!({"type":"response.output_item.added","output_index":0,
                    "item":{"type":"function_call","id":"fc_test","call_id":"call_test","name":"submit_plan","arguments":""}}),
                );
                for part in [args[..args.len() / 2].to_string(), args[args.len() / 2..].to_string()] {
                    body += &sse(
                        "response.function_call_arguments.delta",
                        json!({"type":"response.function_call_arguments.delta","output_index":0,"delta":part}),
                    );
                }
            }
            body += &sse(
                "response.completed",
                json!({"type":"response.completed","response":{"id":"resp_test","status":"completed","model":"test-model","output":[]}}),
            );
            body
        }
        Provider::Claude => {
            let mut body =
                sse("content_block_start", json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}));
            body += &sse(
                "content_block_delta",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Proposition vérifiable."}}),
            );
            body += &sse("content_block_stop", json!({"type":"content_block_stop","index":0}));
            if tool {
                body += &sse(
                    "content_block_start",
                    json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call_test","name":"submit_plan","input":{}}}),
                );
                body += &sse(
                    "content_block_delta",
                    json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":args}}),
                );
                body += &sse("content_block_stop", json!({"type":"content_block_stop","index":1}));
            }
            body += &sse(
                "message_delta",
                json!({"type":"message_delta","delta":{"stop_reason":if tool{"tool_use"}else{"end_turn"}},"usage":{"output_tokens":100}}),
            );
            body += &sse("message_stop", json!({"type":"message_stop"}));
            body
        }
        Provider::Mistral => {
            let mut body = sse(
                "message",
                json!({"choices":[{"index":0,"delta":{"role":"assistant","content":"Proposition vérifiable."},"finish_reason":null}]}),
            );
            if tool {
                body += &sse(
                    "message",
                    json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_test","type":"function","function":{"name":"submit_plan","arguments":args}}]},"finish_reason":null}]}),
                );
            }
            body += &sse("message", json!({"choices":[{"index":0,"delta":{},"finish_reason":if tool{"tool_calls"}else{"stop"}}]}));
            body + "data: [DONE]\n\n"
        }
        Provider::Ollama => {
            let mut m = json!({"role":"assistant","content":"Proposition vérifiable."});
            if tool {
                m["tool_calls"] = json!([{"id":"call_test","function":{"name":"submit_plan","arguments":plan()}}]);
            }
            format!(
                "{}\n{}\n",
                json!({"model":"test-model","message":m,"done":false}),
                json!({"model":"test-model","message":{"content":""},"done":true,"done_reason":"stop"})
            )
        }
    }
}

pub struct Captured {
    pub path: String,
    pub headers: String,
    pub body: Value,
}
pub struct Server {
    pub endpoint: String,
    pub requests: mpsc::Receiver<Captured>,
}
pub fn server(p: Provider, responses: Vec<(u16, String)>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/", listener.local_addr().unwrap());
    let (tx, requests) = mpsc::channel();
    std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        for (status, body) in responses {
            let started = std::time::Instant::now();
            let mut stream = loop {
                if let Ok((s, _)) = listener.accept() {
                    break s;
                }
                if started.elapsed() > Duration::from_secs(15) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 4096];
            let header_end = loop {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
            let length = headers
                .lines()
                .find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok()))
                .unwrap();
            while bytes.len() < header_end + length {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&buf[..n]);
            }
            let path = headers.lines().next().unwrap().split_whitespace().nth(1).unwrap().to_string();
            let captured = Captured { path, headers, body: serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap() };
            if tx.send(captured).is_err() {
                return;
            }
            let content = if p == Provider::Ollama { "application/x-ndjson" } else { "text/event-stream" };
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Type: {content}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            // Deliberately fragment even UTF-8 and JSON across transport writes.
            for part in response.as_bytes().chunks(97) {
                if stream.write_all(part).is_err() {
                    break;
                }
            }
        }
    });
    Server { endpoint, requests }
}
pub fn connection(p: Provider, endpoint: String) -> Connection {
    Connection { model: "test-model".into(), endpoint, tools: true, timeout_secs: 5, ..Connection::for_provider(p) }
}

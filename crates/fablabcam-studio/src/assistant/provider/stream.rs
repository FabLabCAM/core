use super::{public_error, Connection};
use crate::assistant::{Mode, Plan};
use futures_util::StreamExt;
use genai::chat::{ChatOptions, ChatRequest, ChatStreamEvent, StopReason, StreamEnd, ToolResponse};
use std::time::Duration;

pub struct TurnResult {
    pub text: String,
    pub plan: Option<Plan>,
    pub call_id: Option<String>,
    pub request: ChatRequest,
    pub end: StreamEnd,
}
impl TurnResult {
    pub fn continuation(&self, result: &str) -> Result<ChatRequest, String> {
        let id = self.call_id.as_ref().ok_or("Aucun appel en attente")?;
        Ok(self.request.clone().append_tool_use_from_stream_end(&self.end, ToolResponse::new(id, result)))
    }
}

pub async fn chat(
    connection: &Connection,
    secret: &str,
    request: ChatRequest,
    mode: Mode,
    mut chunk: impl FnMut(String),
) -> Result<TurnResult, String> {
    if serde_json::to_vec(&request).map_err(|_| "Requête invalide")?.len() > 256_000 {
        return Err("Conversation supérieure à 256 Ko : commencez une nouvelle demande sans historique".into());
    }
    let client = connection.client(secret)?;
    let options = ChatOptions::default()
        .with_max_tokens(connection.max_tokens)
        .with_capture_content(true)
        .with_capture_tool_calls(true)
        .with_capture_reasoning_content(true)
        .with_stream_header_timeout(Duration::from_secs(connection.timeout_secs))
        .with_stream_read_timeout(Duration::from_secs(connection.timeout_secs));
    let response = client.exec_chat_stream(&connection.model, request.clone(), Some(&options)).await.map_err(|e| public_error(&e))?;
    let mut stream = response.stream;
    let mut text = String::new();
    let mut bytes = 0;
    while let Some(event) = stream.next().await {
        let event = event.map_err(|e| public_error(&e))?;
        bytes += serde_json::to_vec(&event).map_err(|_| "Événement invalide")?.len();
        if bytes > 1_000_000 {
            return Err("Réponse trop volumineuse (1 Mo maximum)".into());
        }
        match event {
            ChatStreamEvent::Chunk(c) => {
                text.push_str(&c.content);
                chunk(c.content);
            }
            ChatStreamEvent::End(end) => return finish(request, mode, text, end),
            _ => {}
        }
    }
    Err("Réponse interrompue avant sa fin : aucun plan applicable".into())
}

fn finish(request: ChatRequest, mode: Mode, text: String, end: StreamEnd) -> Result<TurnResult, String> {
    if !matches!(end.captured_stop_reason, Some(StopReason::Completed(_) | StopReason::ToolCall(_) | StopReason::StopSequence(_))) {
        return Err("Réponse incomplète ou refusée : aucun plan applicable".into());
    }
    let calls = end.captured_tool_calls().unwrap_or_default();
    if calls.len() > 1 {
        return Err("Un seul lot peut être proposé par réponse".into());
    }
    let mut plan = None;
    let mut call_id = None;
    if let Some(call) = calls.first() {
        if mode == Mode::Ask || call.fn_name != "submit_plan" {
            return Err("Appel d'outil refusé par le mode ou le catalogue".into());
        }
        plan = Some(serde_json::from_value::<Plan>(call.fn_arguments.clone()).map_err(|_| "Arguments du plan invalides")?);
        call_id = Some(call.call_id.clone());
    }
    if text.is_empty() && plan.is_none() {
        return Err("Réponse vide du fournisseur".into());
    }
    Ok(TurnResult { text, plan, call_id, request, end })
}

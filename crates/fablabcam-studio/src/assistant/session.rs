use super::{Connection, ContextOptions, ContextSnapshot, Mode, Provider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub user: bool,
    pub text: String,
    pub page: String,
    pub revision: u64,
    pub context_token: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub mode: Mode,
    pub provider: Provider,
    pub connections: Vec<Connection>,
    pub options: ContextOptions,
    pub entries: Vec<Entry>,
    pub debug: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            mode: Mode::Ask,
            provider: Provider::OpenAi,
            connections: Provider::ALL.into_iter().map(Connection::for_provider).collect(),
            options: ContextOptions::default(),
            entries: Vec::new(),
            debug: false,
        }
    }
}

impl Session {
    pub fn connection(&self) -> Connection {
        self.connections.iter().find(|c| c.provider == self.provider).cloned().unwrap_or_else(|| Connection::for_provider(self.provider))
    }
    pub fn set_connection(&mut self, c: Connection) {
        self.connections.retain(|old| old.provider != c.provider);
        self.connections.push(c);
    }
    pub fn push(&mut self, user: bool, text: String, context: &ContextSnapshot, connection: &Connection) {
        self.entries.push(Entry {
            user,
            text,
            page: context.page.clone(),
            revision: context.revision,
            context_token: context.token.clone(),
            provider: connection.identity(),
        });
        if self.entries.len() > 100 {
            self.entries.drain(..self.entries.len() - 100);
        }
    }
}

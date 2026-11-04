//! Native assistant boundary: providers propose; only reviewed domain commands mutate.
pub mod actions;
pub mod context;
pub mod plan;
pub mod provider;
pub mod session;
pub mod storage;
pub mod worker;

pub use context::{ContextOptions, ContextSnapshot, Workspace};
pub use plan::{Action, Mode, Plan, PreparedPlan};
pub use provider::{Connection, Provider};
pub use session::{Entry, Session};

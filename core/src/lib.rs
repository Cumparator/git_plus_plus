// core/src/lib.rs
pub mod types;
pub mod storage;
pub mod backend;
pub mod version_graph;
pub mod push_manager;
pub mod dispatcher;
pub mod plugins;

pub use types::*;
pub use backend::*;
pub use version_graph::*;
// Убрали CommandSpec из списка ниже:
pub use dispatcher::{CommandDispatcher, Command, CmdResult, CommandHandler};
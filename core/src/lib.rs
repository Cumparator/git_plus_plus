pub mod types;
pub mod storage;
pub mod backend;
pub mod version_graph;
pub mod push_manager;
// Новые модули
pub mod dispatcher;
pub mod plugins;

pub use types::*;
pub use backend::*;
pub use version_graph::*;
// Экспортируем диспетчер
pub use dispatcher::{CommandDispatcher, Command, CmdResult, CommandSpec, CommandHandler};
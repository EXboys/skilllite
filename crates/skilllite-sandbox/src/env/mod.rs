//! Runtime environment builder: venv / node_modules for skill execution.
//!
//! Callers (commands, agent) pass skill metadata; this module creates isolated
//! environments and returns paths. Sandbox runner receives only `RuntimePaths`.
//! P0: 优先用系统 Python/Node，系统没有则首次下载到 ~/.skilllite/runtime/，过程透明。

pub mod builder;
pub mod runtime_deps;

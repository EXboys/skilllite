//! Runtime environment builder: venv / node_modules for skill execution.
//!
//! Callers (commands, agent) pass skill metadata; this module creates isolated
//! environments and returns paths. Sandbox runner receives only `RuntimePaths`.

pub mod builder;

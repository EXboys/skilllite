//! IPC parameter structs for stdio JSON-RPC methods.
//!
//! Each struct implements `TryFrom<&Value>` for parsing `params` from JSON-RPC requests,
//! centralizing validation and reducing boilerplate in handlers.

use crate::Error;
use crate::Result;
use serde_json::Value;

fn obj(v: &Value) -> Result<&serde_json::Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| Error::msg("params must be object"))
}

fn req_str(p: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    p.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| Error::msg(format!("{key} required")))
}

fn opt_str(p: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    p.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn opt_bool(p: &serde_json::Map<String, Value>, key: &str) -> bool {
    p.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn opt_u64(p: &serde_json::Map<String, Value>, key: &str) -> Option<u64> {
    p.get(key).and_then(|v| v.as_u64())
}

#[cfg(feature = "agent")]
fn opt_array_strings(p: &serde_json::Map<String, Value>, key: &str) -> Option<Vec<String>> {
    p.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    })
}

/// Parameters for the `run` method.
#[derive(Debug)]
pub struct IpcRunParams {
    pub skill_dir: String,
    pub input_json: String,
    pub allow_network: bool,
    pub cache_dir: Option<String>,
    pub max_memory: Option<u64>,
    pub timeout: Option<u64>,
    pub sandbox_level: Option<u8>,
}

impl TryFrom<&Value> for IpcRunParams {
    type Error = crate::Error;

    fn try_from(v: &Value) -> Result<Self> {
        let p = obj(v)?;
        Ok(IpcRunParams {
            skill_dir: req_str(p, "skill_dir")?,
            input_json: req_str(p, "input_json")?,
            allow_network: opt_bool(p, "allow_network"),
            cache_dir: opt_str(p, "cache_dir"),
            max_memory: opt_u64(p, "max_memory"),
            timeout: opt_u64(p, "timeout"),
            sandbox_level: opt_u64(p, "sandbox_level").map(|u| u as u8),
        })
    }
}

/// Parameters for the `exec` method.
#[derive(Debug)]
pub struct IpcExecParams {
    pub skill_dir: String,
    pub script_path: String,
    pub input_json: String,
    pub args: Option<String>,
    pub allow_network: bool,
    pub cache_dir: Option<String>,
    pub max_memory: Option<u64>,
    pub timeout: Option<u64>,
    pub sandbox_level: Option<u8>,
}

impl TryFrom<&Value> for IpcExecParams {
    type Error = crate::Error;

    fn try_from(v: &Value) -> Result<Self> {
        let p = obj(v)?;
        Ok(IpcExecParams {
            skill_dir: req_str(p, "skill_dir")?,
            script_path: req_str(p, "script_path")?,
            input_json: req_str(p, "input_json")?,
            args: opt_str(p, "args"),
            allow_network: opt_bool(p, "allow_network"),
            cache_dir: opt_str(p, "cache_dir"),
            max_memory: opt_u64(p, "max_memory"),
            timeout: opt_u64(p, "timeout"),
            sandbox_level: opt_u64(p, "sandbox_level").map(|u| u as u8),
        })
    }
}

/// Parameters for the `bash` method.
#[derive(Debug)]
pub struct IpcBashParams {
    pub skill_dir: String,
    pub command: String,
    pub cache_dir: Option<String>,
    pub timeout: u64,
    pub cwd: Option<String>,
}

impl TryFrom<&Value> for IpcBashParams {
    type Error = crate::Error;

    fn try_from(v: &Value) -> Result<Self> {
        let p = obj(v)?;
        Ok(IpcBashParams {
            skill_dir: req_str(p, "skill_dir")?,
            command: req_str(p, "command")?,
            cache_dir: opt_str(p, "cache_dir"),
            timeout: opt_u64(p, "timeout").unwrap_or(120),
            cwd: opt_str(p, "cwd"),
        })
    }
}

/// Parameters for the `build_skills_context` method.
#[derive(Debug)]
#[cfg(feature = "agent")]
pub struct IpcBuildSkillsContextParams {
    pub skills_dir: String,
    pub mode: String,
    pub skills: Option<Vec<String>>,
}

#[cfg(feature = "agent")]
impl TryFrom<&Value> for IpcBuildSkillsContextParams {
    type Error = crate::Error;

    fn try_from(v: &Value) -> Result<Self> {
        let p = obj(v)?;
        Ok(IpcBuildSkillsContextParams {
            skills_dir: req_str(p, "skills_dir")?,
            mode: opt_str(p, "mode").unwrap_or_else(|| "progressive".into()),
            skills: opt_array_strings(p, "skills"),
        })
    }
}

/// Parameters for the `list_tools` method.
#[derive(Debug)]
#[cfg(feature = "agent")]
pub struct IpcListToolsParams {
    pub skills_dir: String,
    pub skills: Option<Vec<String>>,
    pub format: String,
}

#[cfg(feature = "agent")]
impl TryFrom<&Value> for IpcListToolsParams {
    type Error = crate::Error;

    fn try_from(v: &Value) -> Result<Self> {
        let p = obj(v)?;
        Ok(IpcListToolsParams {
            skills_dir: req_str(p, "skills_dir")?,
            skills: opt_array_strings(p, "skills"),
            format: opt_str(p, "format").unwrap_or_else(|| "openai".into()),
        })
    }
}

//! `agent-sandbox` command. Streams redacted stdout from `skilllite_sandbox::run`.

use std::io::{self, Write};
use std::path::PathBuf;

use skilllite_sandbox::{run, RunRequest};

use crate::cli::Commands;
use crate::command_registry::CommandRegistry;

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::AgentSandbox {
            account_dir,
            work_dir,
            prompt,
            https_proxy,
            require_proxy,
            run_id,
        } = cmd
        {
            Some(run_command(
                account_dir,
                work_dir,
                prompt,
                https_proxy,
                *require_proxy,
                run_id.as_deref(),
            ))
        } else {
            None
        }
    });
}

fn run_command(
    account_dir: &str,
    work_dir: &str,
    prompt: &str,
    https_proxy: &str,
    require_proxy: bool,
    run_id: Option<&str>,
) -> crate::Result<()> {
    let mut request = RunRequest::new(
        PathBuf::from(account_dir),
        PathBuf::from(work_dir),
        prompt,
        https_proxy,
    );
    request.require_proxy = require_proxy;
    if let Some(id) = run_id {
        request.run_id = id.to_string();
    }
    let mut stdout = io::stdout().lock();
    let outcome = run(request, |line| {
        let _ = writeln!(stdout, "{line}");
        let _ = stdout.flush();
    })?;
    writeln!(
        stdout,
        "{{\"type\":\"runtime\",\"exit_code\":{}}}",
        outcome.exit_code
    )?;
    if outcome.exit_code != 0 {
        return Err(crate::Error::msg(format!(
            "agent sandbox exited with code {}",
            outcome.exit_code
        )));
    }
    Ok(())
}

//! 技能管理命令：Add, Remove, List, ListTools, Show, Verify

use crate::cli::Commands;
use crate::command_registry::CommandRegistry;
use crate::stdio_rpc;

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Add {
            source,
            skills_dir,
            force,
            list,
            scan_offline,
        } = cmd
        {
            Some(skilllite_commands::skill::cmd_add(
                source,
                skills_dir,
                *force,
                *list,
                *scan_offline,
            ))
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Remove {
            skill_name,
            skills_dir,
            force,
        } = cmd
        {
            Some(skilllite_commands::skill::cmd_remove(
                skill_name,
                skills_dir,
                *force,
            ))
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::List {
            skills_dir,
            json,
            scan,
        } = cmd
        {
            Some(skilllite_commands::skill::cmd_list(skills_dir, *json, *scan))
        } else {
            None
        }
    });

    #[cfg(feature = "agent")]
    reg.register(|cmd| {
        if let Commands::ListTools { skills_dir, format } = cmd {
            let r = (|| {
                let params = serde_json::json!({
                    "skills_dir": skills_dir,
                    "format": format
                });
                let result = stdio_rpc::handle_list_tools(&params)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
                Ok::<(), anyhow::Error>(())
            })();
            Some(r)
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Show {
            skill_name,
            skills_dir,
            json,
        } = cmd
        {
            Some(skilllite_commands::skill::cmd_show(
                skill_name,
                skills_dir,
                *json,
            ))
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Verify {
            target,
            skills_dir,
            json,
            strict,
        } = cmd
        {
            Some(skilllite_commands::skill::cmd_verify(
                target,
                skills_dir,
                *json,
                *strict,
            ))
        } else {
            None
        }
    });
}

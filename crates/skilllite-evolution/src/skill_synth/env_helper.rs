//! 验证/修复前确保技能依赖已安装：无 package.json/requirements.txt 时从 SKILL.md compatibility 推断并安装。

use std::path::{Path, PathBuf};

use skilllite_core::skill::dependency_resolver;
use skilllite_core::skill::metadata;
use skilllite_core::EnvSpec;
use skilllite_sandbox::env::builder as env_builder;

/// 解析 metadata，若无 resolved_packages 但有 compatibility 则用 whitelist 解析依赖并安装环境。
/// 返回隔离环境目录路径；无需环境或失败时返回 None。
pub(super) fn ensure_skill_deps_and_env(skill_dir: &Path) -> Option<PathBuf> {
    let mut meta = metadata::parse_skill_metadata(skill_dir).ok()?;
    // 无 lock/无 package 时，从 compatibility 推断依赖（与大模型在 SKILL.md 里写的兼容性描述一致）
    if meta.resolved_packages.is_none()
        && meta.compatibility.as_ref().map_or(false, |c| !c.trim().is_empty())
    {
        let lang = metadata::detect_language(skill_dir, &meta);
        if let Ok(resolved) = dependency_resolver::resolve_packages_sync(
            skill_dir,
            meta.compatibility.as_deref(),
            &lang,
            true,
        ) {
            if !resolved.packages.is_empty() {
                meta.resolved_packages = Some(resolved.packages);
            }
        }
    }
    let env_spec = EnvSpec::from_metadata(skill_dir, &meta);
    env_builder::ensure_environment(skill_dir, &env_spec, None).ok()
}

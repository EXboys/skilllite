//! Windows: suppress visible console windows when spawning CLI children from the GUI host.

#[cfg(windows)]
pub(crate) fn hide_child_console(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn hide_child_console(_cmd: &mut std::process::Command) {}

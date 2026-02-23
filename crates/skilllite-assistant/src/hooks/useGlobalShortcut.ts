import { useEffect } from "react";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWindow } from "@tauri-apps/api/window";

const SHORTCUT = "CommandOrControl+Shift+L";

export function useGlobalShortcut() {
  useEffect(() => {
    register(SHORTCUT, async () => {
      const win = getCurrentWindow();
      const visible = await win.isVisible();
      if (visible) {
        await win.hide();
      } else {
        await win.show();
        await win.setFocus();
      }
    });

    return () => {
      unregister(SHORTCUT);
    };
  }, []);
}

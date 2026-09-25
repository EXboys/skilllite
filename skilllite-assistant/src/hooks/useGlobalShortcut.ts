import { useEffect } from "react";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { translate } from "../i18n";

const SHORTCUT = "CommandOrControl+Shift+L";

export function useGlobalShortcut() {
  useEffect(() => {
    let cancelled = false;
    register(SHORTCUT, async () => {
      const win = getCurrentWindow();
      const visible = await win.isVisible();
      if (visible) {
        await win.hide();
      } else {
        await win.show();
        await win.setFocus();
      }
    })
      .then(() => {
        if (cancelled) return;
      })
      .catch((e) => {
        if (cancelled) return;
        useUiToastStore
          .getState()
          .show(
            translate("toast.shortcutFailed", {
              shortcut: SHORTCUT,
              err: formatInvokeError(e),
            }),
            "error"
          );
      });

    return () => {
      cancelled = true;
      unregister(SHORTCUT).catch(() => {});
    };
  }, []);
}

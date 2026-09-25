import { create } from "zustand";

/**
 * Chat read_file → IDE middle editor: MainLayout subscribes and clears after applying.
 */
interface IdeFileOpenerState {
  pendingRelativePath: string | null;
  openFileFromChat: (relativePath: string) => void;
  clearPending: () => void;
}

export const useIdeFileOpenerStore = create<IdeFileOpenerState>((set) => ({
  pendingRelativePath: null,
  openFileFromChat: (relativePath) => {
    const p = relativePath.trim();
    if (p) set({ pendingRelativePath: p });
  },
  clearPending: () => set({ pendingRelativePath: null }),
}));

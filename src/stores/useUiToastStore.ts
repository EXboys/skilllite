import { create } from "zustand";

export type UiToastVariant = "error" | "info";

interface UiToastState {
  message: string | null;
  variant: UiToastVariant;
  show: (message: string, variant?: UiToastVariant) => void;
  clear: () => void;
}

export const useUiToastStore = create<UiToastState>((set) => ({
  message: null,
  variant: "error",
  show: (message, variant = "error") => set({ message, variant }),
  clear: () => set({ message: null }),
}));

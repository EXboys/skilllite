export interface TaskItem {
  id: number;
  description: string;
  tool_hint?: string;
  completed?: boolean;
}

export type ChatMessage =
  | { id: string; type: "user"; content: string }
  | { id: string; type: "assistant"; content: string; streaming?: boolean }
  | { id: string; type: "plan"; tasks: TaskItem[] }
  | { id: string; type: "tool_call"; name: string; args: string }
  | { id: string; type: "tool_result"; name: string; result: string; isError: boolean }
  | {
      id: string;
      type: "confirmation";
      prompt: string;
      resolved?: boolean;
      approved?: boolean;
    };

export interface StreamEventPayload {
  event: string;
  data: Record<string, unknown>;
}

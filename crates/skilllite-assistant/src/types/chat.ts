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
  | {
      id: string;
      type: "tool_result";
      name: string;
      result: string;
      isError: boolean;
      /** 来自同轮 read_file 工具调用的 path（用于全屏保存） */
      sourcePath?: string;
    }
  | {
      id: string;
      type: "confirmation";
      prompt: string;
      resolved?: boolean;
      approved?: boolean;
    }
  | {
      id: string;
      type: "clarification";
      reason: string;
      message: string;
      suggestions: string[];
      resolved?: boolean;
      selectedOption?: string;
    }
  | {
      id: string;
      type: "evolution_options";
      toolName: string;
      outcome: "partial_success" | "failure";
      message: string;
      options: string[];
      resolved?: boolean;
      selectedOption?: string;
      proposalId?: string;
      progressStatus?: string;
      progressUpdatedAt?: string;
      progressNote?: string;
      progressDone?: boolean;
    };

export interface StreamEventPayload {
  event: string;
  data: Record<string, unknown>;
  session_key?: string;
}

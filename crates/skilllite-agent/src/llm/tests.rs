//! Tests for the LLM client.

use super::*;
use crate::types::{ChatMessage, FunctionCall, ToolCall};
use serde_json::json;


#[test]
fn test_detect_tool_format_openai() {
    assert_eq!(
        detect_tool_format("gpt-4o", "https://api.openai.com/v1"),
        ToolFormat::OpenAI
    );
    assert_eq!(
        detect_tool_format("deepseek-chat", "https://api.deepseek.com/v1"),
        ToolFormat::OpenAI
    );
    assert_eq!(
        detect_tool_format("qwen-turbo", "https://dashscope.aliyuncs.com/v1"),
        ToolFormat::OpenAI
    );
}

#[test]
fn test_detect_tool_format_claude() {
    assert_eq!(
        detect_tool_format("claude-3-5-sonnet-20241022", "https://api.anthropic.com"),
        ToolFormat::Claude
    );
    assert_eq!(
        detect_tool_format("claude-3-opus", "https://custom.proxy.com"),
        ToolFormat::Claude
    );
    assert_eq!(
        detect_tool_format("gpt-4o", "https://anthropic-proxy.example.com/v1"),
        ToolFormat::Claude
    );
}

#[test]
fn test_convert_messages_for_claude_basic() {
    let messages = vec![
        ChatMessage::system("You are helpful."),
        ChatMessage::user("Hello"),
        ChatMessage::assistant("Hi there!"),
    ];

    let (system, claude_msgs) = LlmClient::convert_messages_for_claude(&messages);

    assert_eq!(system, Some("You are helpful.".to_string()));
    assert_eq!(claude_msgs.len(), 2); // user + assistant (system extracted)

    assert_eq!(claude_msgs[0]["role"], "user");
    assert_eq!(claude_msgs[0]["content"], "Hello");

    assert_eq!(claude_msgs[1]["role"], "assistant");
    let content = claude_msgs[1]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "Hi there!");
}

#[test]
fn test_convert_messages_for_claude_tool_calls() {
    let tool_call = ToolCall {
        id: "tc_123".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "read_file".to_string(),
            arguments: r#"{"path": "test.txt"}"#.to_string(),
        },
    };

    let messages = vec![
        ChatMessage::system("System prompt"),
        ChatMessage::user("Read the file"),
        ChatMessage::assistant_with_tool_calls(Some("Let me read that."), vec![tool_call]),
        ChatMessage::tool_result("tc_123", "File contents here"),
    ];

    let (system, claude_msgs) = LlmClient::convert_messages_for_claude(&messages);

    assert!(system.is_some());
    assert_eq!(claude_msgs.len(), 3); // user, assistant (with tool_use), user (with tool_result)

    // Check assistant has both text and tool_use blocks
    let assistant_content = claude_msgs[1]["content"].as_array().unwrap();
    assert_eq!(assistant_content.len(), 2);
    assert_eq!(assistant_content[0]["type"], "text");
    assert_eq!(assistant_content[1]["type"], "tool_use");
    assert_eq!(assistant_content[1]["id"], "tc_123");
    assert_eq!(assistant_content[1]["name"], "read_file");

    // Check tool result is wrapped as user message
    let tool_result_msg = &claude_msgs[2];
    assert_eq!(tool_result_msg["role"], "user");
    let result_content = tool_result_msg["content"].as_array().unwrap();
    assert_eq!(result_content[0]["type"], "tool_result");
    assert_eq!(result_content[0]["tool_use_id"], "tc_123");
    assert_eq!(result_content[0]["content"], "File contents here");
}

#[test]
fn test_convert_messages_for_claude_multiple_tool_results() {
    let tc1 = ToolCall {
        id: "tc_1".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "tool_a".to_string(),
            arguments: "{}".to_string(),
        },
    };
    let tc2 = ToolCall {
        id: "tc_2".to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: "tool_b".to_string(),
            arguments: "{}".to_string(),
        },
    };

    let messages = vec![
        ChatMessage::system("sys"),
        ChatMessage::user("do both"),
        ChatMessage::assistant_with_tool_calls(None, vec![tc1, tc2]),
        ChatMessage::tool_result("tc_1", "result a"),
        ChatMessage::tool_result("tc_2", "result b"),
    ];

    let (_, claude_msgs) = LlmClient::convert_messages_for_claude(&messages);

    // Multiple tool results should be batched into one user message
    assert_eq!(claude_msgs.len(), 3); // user, assistant, user(tool_results)

    let result_content = claude_msgs[2]["content"].as_array().unwrap();
    assert_eq!(result_content.len(), 2);
    assert_eq!(result_content[0]["tool_use_id"], "tc_1");
    assert_eq!(result_content[1]["tool_use_id"], "tc_2");
}

#[test]
fn test_convert_claude_response() {
    let response = json!({
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "Here is the result."}
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50
        }
    });

    let result = LlmClient::convert_claude_response(response, "claude-3-5-sonnet").unwrap();

    assert_eq!(result.choices.len(), 1);
    assert_eq!(
        result.choices[0].message.content,
        Some("Here is the result.".to_string())
    );
    assert!(result.choices[0].message.tool_calls.is_none());
    assert_eq!(result.choices[0].finish_reason, Some("stop".to_string()));
    assert!(result.usage.is_some());
    assert_eq!(result.usage.unwrap().total_tokens, 150);
}

#[test]
fn test_convert_claude_response_with_tool_use() {
    let response = json!({
        "id": "msg_456",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "text", "text": "Let me read that file."},
            {
                "type": "tool_use",
                "id": "toolu_01",
                "name": "read_file",
                "input": {"path": "hello.txt"}
            }
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 50, "output_tokens": 30}
    });

    let result = LlmClient::convert_claude_response(response, "claude-3-5-sonnet").unwrap();

    assert_eq!(
        result.choices[0].message.content,
        Some("Let me read that file.".to_string())
    );
    let tool_calls = result.choices[0].message.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "toolu_01");
    assert_eq!(tool_calls[0].function.name, "read_file");
    assert!(tool_calls[0].function.arguments.contains("hello.txt"));
    assert_eq!(
        result.choices[0].finish_reason,
        Some("tool_calls".to_string())
    );
}

#[test]
fn test_is_context_overflow_error() {
    assert!(is_context_overflow_error("context_length_exceeded"));
    assert!(is_context_overflow_error("Maximum context length exceeded"));
    assert!(is_context_overflow_error("too many tokens in request"));
    assert!(!is_context_overflow_error("rate limit exceeded"));
    assert!(!is_context_overflow_error("invalid api key"));
}

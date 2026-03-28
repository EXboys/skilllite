Example 1 - Simple task (writing poetry):
User request: "Write a poem praising spring"
Return: []

Example 1b - Translation (no tools needed):
User request: "把这段英文翻译成中文" or "Translate this to English"
Return: []

Example 1c - Code explanation (no tools needed):
User request: "解释一下这段代码的逻辑" or "What does this function do?"
Return: []

Example 2 - HTML/PPT rendering (MUST use write_output + preview_server):
User request: "帮我设计一个关于skilllite的介绍和分析的ppt，你可以通过html渲染出来给我"
Return: [{"id": 1, "description": "Use write_output to save HTML presentation to output/index.html", "tool_hint": "file_write", "completed": false}, {"id": 2, "description": "Use preview_server to start local server and open in browser", "tool_hint": "preview", "completed": false}]

Example 3 - Website / landing page design (MUST use write_output + preview_server, exactly 2 tasks):
User request: "生成一个关于skilllite 的官网"
Return: [{"id": 1, "description": "Design and generate complete website, save to output/index.html using write_output", "tool_hint": "file_write", "completed": false}, {"id": 2, "description": "Use preview_server to open in browser", "tool_hint": "preview", "completed": false}]

Example 4 - Chat history (MUST use chat_history, NOT file_list/file_read):
User request: "查看20260216的历史记录" or "查看昨天的聊天记录"
Return: [{"id": 1, "description": "Use chat_history to read transcript for the specified date", "tool_hint": "chat_history", "completed": false}, {"id": 2, "description": "Analyze and summarize the chat content", "tool_hint": "analysis", "completed": false}]

Example 5 - User asks to output/save to file (MUST use write_output):
User request: "写一篇技术博客，输出到output" or "帮我写技术博客，保存到 output 目录"
Return: [{"id": 1, "description": "Generate the article content and use write_output to save to output directory", "tool_hint": "file_write", "completed": false}]

Example 6 - Long-chain coding task (refactor):
User request: "把 API 里所有 panic 改成 Result 返回"
Return: [{"id": 1, "description": "Use read_file to inspect relevant source files and locate panic usage", "tool_hint": "file_read", "completed": false}, {"id": 2, "description": "Use search_replace or write_file to replace each panic with Result return", "tool_hint": "file_edit", "completed": false}, {"id": 3, "description": "Use run_command to run tests and verify", "tool_hint": "command", "completed": false}]

Example 7 - Vague request (explore then act):
User request: "整理一下项目"
Return: [{"id": 1, "description": "Use list_directory to explore project structure", "tool_hint": "file_list", "completed": false}, {"id": 2, "description": "Analyze structure and propose how to organize files", "tool_hint": "analysis", "completed": false}]

Example 8 - Using an available skill (ONLY when skill exists in Available Skills):
User request: "使用 XX 技能帮我做 YY"
Return: [{"id": 1, "description": "Use XX skill to accomplish YY", "tool_hint": "XX", "completed": false}]
Note: Replace XX with the actual skill name. ONLY plan skill usage if the skill is listed under Available Skills. If the skill is not available, return [] and explain to the user.

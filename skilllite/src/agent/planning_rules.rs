//! Planning rules for task generation.
//!
//! Single source of truth for built-in rules. Edit this file to add or modify rules.

use super::types::PlanningRule;

/// Built-in planning rules. Used when no external override is configured.
pub fn builtin_rules() -> Vec<PlanningRule> {
    vec![
        PlanningRule {
            id: "explicit_skill".into(),
            priority: 100,
            keywords: vec![],
            context_keywords: vec![],
            tool_hint: None,
            instruction: "**If user says \"使用 XX skill\" / \"用 XX 技能\" / \"use XX skills\"**, you MUST add that skill to the task list. Do NOT return empty list.".into(),
        },
        PlanningRule {
            id: "weather".into(),
            priority: 90,
            keywords: vec![
                "天气".into(),
                "气温".into(),
                "气象".into(),
                "今天天气".into(),
                "明天天气".into(),
                "适合出行吗".into(),
                "适合出去玩吗".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("weather".into()),
            instruction: "**天气/气象/天气预报**: When the user asks about weather, you MUST use **weather** skill. The LLM cannot provide real-time weather data; only the weather skill can. Return a task with tool_hint: \"weather\".".into(),
        },
        PlanningRule {
            id: "realtime_http".into(),
            priority: 90,
            keywords: vec![
                "实时".into(),
                "最新".into(),
                "实时信息".into(),
                "最新数据".into(),
                "实时数据".into(),
                "最新排名".into(),
                "实时查询".into(),
                "抓取网页".into(),
                "获取最新".into(),
                "fetch live data".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("http-request".into()),
            instruction: "**实时/最新/实时信息**: When the user explicitly asks for real-time or latest data, you MUST use **http-request** skill. The LLM's knowledge has a cutoff; only HTTP requests can fetch current information. Return a task with tool_hint: \"http-request\".".into(),
        },
        PlanningRule {
            id: "external_comparison".into(),
            priority: 92,
            keywords: vec![
                "对比".into(),
                "比较".into(),
                "优劣势".into(),
                "优劣对比".into(),
                "对比分析".into(),
                "全方位".into(),
                "全方位分析".into(),
                "vs".into(),
                "versus".into(),
            ],
            context_keywords: vec!["地方".into(), "城市".into(), "两地".into(), "城市对比".into()],
            tool_hint: Some("http-request".into()),
            instruction: "**对比/比较/优劣势** (places, cities, companies, topics): When the user asks to compare or analyze pros/cons of places, cities, companies, or external topics, PREFER **http-request** to fetch fresh data. Do NOT use chat_history — it is ONLY for past chat/conversation analysis. Plan: (1) http-request to fetch current info from web; (2) analysis. If user did not ask for 实时/最新 and task is general knowledge, you may return [] to let LLM answer directly.".into(),
        },
        PlanningRule {
            id: "continue_context".into(),
            priority: 85,
            keywords: vec!["继续".into(), "继续未完成".into(), "继续之前".into(), "继续任务".into()],
            context_keywords: vec![
                "实时".into(),
                "最新".into(),
                "排名".into(),
                "university".into(),
                "QS".into(),
                "官网".into(),
                "需要用户自行查询".into(),
                "请访问官网".into(),
            ],
            tool_hint: Some("http-request".into()),
            instruction: "**继续/继续未完成的任务**: When the user says 继续, you MUST use the **conversation context** to understand what task to continue. If the context mentions real-time data, rankings, or similar, plan **http-request** to fetch the data.".into(),
        },
        PlanningRule {
            id: "xiaohongshu".into(),
            priority: 90,
            keywords: vec!["小红书".into(), "种草文案".into(), "小红书图文".into(), "小红书笔记".into()],
            context_keywords: vec![],
            tool_hint: Some("xiaohongshu-writer".into()),
            instruction: "**小红书/种草/图文笔记**: When the task involves 小红书 content, you MUST use **xiaohongshu-writer** skill.".into(),
        },
        PlanningRule {
            id: "frontend_design".into(),
            priority: 92,
            keywords: vec![
                "官网".into(),
                "网站".into(),
                "网站设计".into(),
                "设计网页".into(),
                "设计页面".into(),
                "前端设计".into(),
                "页面设计".into(),
                "landing page".into(),
                "website".into(),
                "web page".into(),
                "homepage".into(),
                "首页".into(),
                "网站首页".into(),
                "官方网站".into(),
                "做个网站".into(),
                "做一个网站".into(),
                "生成网站".into(),
                "生成页面".into(),
                "生成网页".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("file_operation".into()),
            instruction: "**官网/网站/网页设计**: When the user asks to design or generate a website, landing page, or web page, you MUST plan exactly TWO tasks: (1) Generate the complete HTML/CSS/JS and use **write_output** to save to index.html (tool_hint: file_operation); (2) Use **preview_server** to start local server and open in browser (tool_hint: file_operation). If a frontend-design skill exists, it is reference-only — use its design guidelines but output via write_output. Do NOT call the frontend-design skill directly. Do NOT return empty list — website generation requires file output + preview.".into(),
        },
        PlanningRule {
            id: "html_preview".into(),
            priority: 90,
            keywords: vec![
                "html渲染".into(),
                "渲染出来".into(),
                "预览".into(),
                "在浏览器中打开".into(),
                "html呈现".into(),
                "网页渲染".into(),
                "PPT".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("file_operation".into()),
            instruction: "**HTML/PPT/渲染/预览**: When the user asks for HTML rendering or browser preview, use **write_output** + **preview_server**.".into(),
        },
        PlanningRule {
            id: "chat_history".into(),
            priority: 95,
            keywords: vec![
                "历史记录".into(),
                "聊天记录".into(),
                "聊天历史".into(),
                "查看记录".into(),
                "chat history".into(),
                "conversation history".into(),
                "past chat".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("chat_history".into()),
            instruction: "**历史记录/聊天记录**: When the user asks to view, summarize, or analyze past chat/conversation history, you MUST use **chat_history** (built-in). Do NOT use list_directory or file_operation — chat_history reads directly from transcripts. Plan: (1) Use chat_history with date if specified; (2) Analyze/summarize the content.".into(),
        },
        PlanningRule {
            id: "analyze_stability".into(),
            priority: 85,
            keywords: vec![
                "分析稳定性".into(),
                "分析项目问题".into(),
                "分析历史消息".into(),
                "分析健壮性".into(),
                "分析最近".into(),
                "ai稳定性".into(),
                "项目问题".into(),
                "历史消息".into(),
                "最近几次".into(),
                "健壮性".into(),
                "analyze stability".into(),
                "project issues".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("chat_history".into()),
            instruction: "**分析AI稳定性/项目问题/历史消息** (ONLY when user explicitly asks to analyze chat/conversation): When the user asks to analyze recent AI stability, project issues, robustness, or conversation quality, you MUST use **chat_history** first to get the data, then analyze. chat_history is ONLY for analyzing past chat records — do NOT use it for comparing places, cities, companies, or external topics.".into(),
        },
        PlanningRule {
            id: "output_to_file".into(),
            priority: 92,
            keywords: vec![
                "输出到".into(),
                "输出到output".into(),
                "保存到".into(),
                "写到文件".into(),
                "写入文件".into(),
                "保存为".into(),
                "输出到文件".into(),
                "save to".into(),
                "output to".into(),
                "write to file".into(),
            ],
            context_keywords: vec![],
            tool_hint: Some("file_operation".into()),
            instruction: "**输出到 output/文件**: When the user explicitly asks to output, save, or write content to a file (e.g. 输出到output, 保存到文件, 写到 output), you MUST plan a file_operation task using **write_output**. Even if the content is an article, report, or markdown, saving to file requires the tool. Return a task with tool_hint: \"file_operation\" and description like \"Use write_output to save the generated content to output/<filename>\".".into(),
        },
    ]
}

/// Full examples section (all 11 examples) for non-compact mode.
pub fn full_examples_section() -> String {
    r#"Example 1 - Simple task (writing poetry):
User request: "Write a poem praising spring"
Return: []

Example 2 - Task requiring tools:
User request: "Calculate 123 * 456 + 789 for me"
Return: [{"id": 1, "description": "Use calculator to compute expression", "tool_hint": "calculator", "completed": false}]

Example 3 - User explicitly asks to use a skill (MUST use that skill):
User request: "写一个关于本项目推广的小红书的图文，使用小红书的skills"
Return: [{"id": 1, "description": "Use xiaohongshu-writer to generate 小红书 content with thumbnail", "tool_hint": "xiaohongshu-writer", "completed": false}]

Example 4 - Weather query (MUST use weather skill, LLM cannot provide real-time data):
User request: "深圳今天天气怎样，适合出去玩吗？"
Return: [{"id": 1, "description": "Use weather skill to query real-time weather in Shenzhen", "tool_hint": "weather", "completed": false}]

Example 5 - User asks for real-time/latest info (MUST use http-request):
User request: "我需要更实时的信息" or "分析西安交大和清迈大学的对比，要最新数据"
Return: [{"id": 1, "description": "Use http-request to fetch latest data from authoritative sources (QS, official sites)", "tool_hint": "http-request", "completed": false}, {"id": 2, "description": "Analyze and compare based on fetched data", "tool_hint": "analysis", "completed": false}]

Example 5b - User asks to compare places/cities (use http-request for fresh data, NOT chat_history):
User request: "分析一下清迈和深圳这两个地方的优劣势对比" or "比较北京和上海"
Return: [{"id": 1, "description": "Use http-request to fetch current information about both places", "tool_hint": "http-request", "completed": false}, {"id": 2, "description": "Analyze and compare based on fetched data", "tool_hint": "analysis", "completed": false}]
Note: chat_history is for past CONVERSATION only. Do NOT use it for place/city/topic comparison.

Example 6 - User says "继续" with context (MUST use context to infer task):
User request: "继续为我那未完成的任务"
Conversation context: [assistant previously said: "要完成西安交大与清迈大学的对比..."]
Return: [{"id": 1, "description": "Use http-request to fetch QS rankings...", "tool_hint": "http-request", "completed": false}, {"id": 2, "description": "Analyze and present comparison", "tool_hint": "analysis", "completed": false}]

Example 7 - HTML/PPT rendering (MUST use write_output + preview_server):
User request: "帮我设计一个关于skilllite的介绍和分析的ppt，你可以通过html渲染出来给我"
Return: [{"id": 1, "description": "Use write_output to save HTML presentation to output/index.html", "tool_hint": "file_operation", "completed": false}, {"id": 2, "description": "Use preview_server to start local server and open in browser", "tool_hint": "file_operation", "completed": false}]

Example 8 - Website / landing page design (MUST use write_output + preview_server, exactly 2 tasks):
User request: "生成一个关于skilllite 的官网"
Return: [{"id": 1, "description": "Design and generate complete website, save to output/index.html using write_output", "tool_hint": "file_operation", "completed": false}, {"id": 2, "description": "Use preview_server to open in browser", "tool_hint": "file_operation", "completed": false}]

Example 9 - Chat history (MUST use chat_history, NOT list_directory or file_operation):
User request: "查看20260216的历史记录" or "查看昨天的聊天记录"
Return: [{"id": 1, "description": "Use chat_history to read transcript for the specified date", "tool_hint": "chat_history", "completed": false}, {"id": 2, "description": "Analyze and summarize the chat content", "tool_hint": "analysis", "completed": false}]

Example 10 - User asks to output/save to file (MUST use write_output, even for articles):
User request: "写一篇CSDN文章，输出到output" or "帮我写技术博客，保存到 output 目录"
Return: [{"id": 1, "description": "Generate the article content and use write_output to save to output directory", "tool_hint": "file_operation", "completed": false}]

Example 11 - User asks to analyze AI stability / project issues (MUST use chat_history, NOT write_output):
User request: "分析一下最近几次的ai的稳定性以及项目的问题" or "分析历史消息的健壮性"
Return: [{"id": 1, "description": "Use chat_history to read recent conversation transcripts", "tool_hint": "chat_history", "completed": false}, {"id": 2, "description": "Analyze AI stability and project issues based on the transcripts", "tool_hint": "analysis", "completed": false}]
Note: The user wants ANALYSIS of existing data, NOT a new article. Do NOT plan write_output."#
        .to_string()
}

/// Compact examples section: core examples + up to 3 matched by user message keywords.
pub fn compact_examples_section(user_message: &str) -> String {
    let msg_lower = user_message.to_lowercase();
    let mut lines = vec![
        "Example 1 - Simple (no tools): \"Write a poem\" → []".to_string(),
        "Example 2 - Tools: \"Calculate 123*456\" → [{\"id\":1,\"description\":\"Use calculator\",\"tool_hint\":\"calculator\",\"completed\":false}]".to_string(),
    ];
    // Detect city/place comparison context: chat_history is WRONG for these
    let is_city_or_place = user_message.contains("城市")
        || user_message.contains("地方")
        || user_message.contains("对比")
        || user_message.contains("优劣势")
        || user_message.contains("全方位")
        || user_message.contains("两地")
        || msg_lower.contains("city")
        || msg_lower.contains("place");
    let candidates: Vec<(&str, &str, &str)> = vec![
        ("城市", "全方位", "城市/地方/全方位分析: http-request for fresh data. NOT chat_history."),
        ("对比", "优劣势", "对比/优劣势: http-request for fresh data. NOT chat_history."),
        ("分析", "稳定性", "分析稳定性/项目: chat_history (ONLY when analyzing chat/project, NOT places)"),
        ("历史", "记录", "历史记录: chat_history + analysis."),
        ("输出到", "保存到", "输出到output: write_output, file_operation."),
        ("继续", "", "继续: use context to infer task, often http-request."),
        ("天气", "气象", "天气: weather skill."),
        ("官网", "网站", "官网/网站: write_output + preview_server, 2 tasks."),
    ];
    let mut added = 0;
    for (k1, k2, text) in candidates {
        if added >= 3 {
            break;
        }
        let matches = user_message.contains(k1)
            || msg_lower.contains(&k1.to_lowercase())
            || (!k2.is_empty() && (user_message.contains(k2) || msg_lower.contains(&k2.to_lowercase())));
        // Skip 分析/稳定性 when context is city/place — avoid steering toward chat_history
        let skip = matches
            && k1 == "分析"
            && is_city_or_place;
        if matches && !skip {
            lines.push(format!("Example - {}: {}", k1, text));
            added += 1;
        }
    }
    lines.join("\n")
}

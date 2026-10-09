pub fn resolve(provider: &str) -> (&'static str, &'static str) {
    match provider.to_lowercase().as_str() {
        "openai" => ("https://api.openai.com/v1/chat/completions", "gpt-4o-mini"),
        _ => ("https://api.deepseek.com/chat/completions", "deepseek-chat"),
    }
}

fn target_label(target: &str) -> &'static str {
    match target.to_lowercase().as_str() {
        "en" => "英语",
        "ja" => "日语",
        "ko" => "韩语",
        _ => "中文",
    }
}

pub fn translate_lines(
    provider: &str,
    api_key: &str,
    lines: &[String],
    target: &str,
) -> Result<Vec<String>, String> {
    if lines.is_empty() {
        return Ok(Vec::new());
    }
    let (url, model) = resolve(provider);
    let label = target_label(target);

    let numbered: String = lines
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{}. {}", i + 1, l))
        .collect::<Vec<_>>()
        .join("\n");

    let system = format!(
        "你是专业的影视字幕翻译。把用户提供的每条字幕翻译成{label}。\
         严格保持条目数量和顺序一一对应，禁止合并、拆分或增删条目。\
         只输出一个 JSON 数组（形如 [\"翻译1\",\"翻译2\"]），不要输出任何其他文字。"
    );

    let body = serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": numbered }
        ]
    });

    let resp = ureq::post(url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .send_json(&body)
        .map_err(|e| format!("翻译请求失败: {e}"))?;

    let v: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("解析翻译响应失败: {e}"))?;

    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("翻译服务未返回内容")?;

    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: serde_json::Value =
        serde_json::from_str(cleaned).map_err(|_| "翻译结果不是有效 JSON".to_string())?;

    let arr = parsed
        .as_array()
        .ok_or("翻译结果应为 JSON 数组")?;
    let mut out: Vec<String> = Vec::new();
    for item in arr {
        let s = item.as_str().ok_or("翻译结果包含非字符串")?.trim().to_string();
        out.push(s);
    }
    if out.len() != lines.len() {
        return Err(format!(
            "翻译结果条数不符（期望 {}，实际 {}），请重试",
            lines.len(),
            out.len()
        ));
    }
    Ok(out)
}
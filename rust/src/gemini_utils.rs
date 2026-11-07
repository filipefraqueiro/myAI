use std::collections::HashMap;
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::structs::{
    Node,
    Message,
};
use crate::utils::strip_thinking;

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn push_content(contents: &mut Vec<Value>, role: &str, mut parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    // Merge consecutive same-role turns (e.g. several tool results in a row)
    if let Some(last) = contents.last_mut() {
        if last["role"] == role {
            if let Some(existing) = last["parts"].as_array_mut() {
                existing.append(&mut parts);
                return;
            }
        }
    }
    contents.push(json!({ "role": role, "parts": parts }));
}

/// OpenAI-style messages -> (systemInstruction, contents) for Gemini.
pub fn messages_to_gemini(messages: &Value) -> (Option<Value>, Vec<Value>) {
    let mut system_texts: Vec<String> = Vec::new();
    let mut contents: Vec<Value> = Vec::new();
    // Gemini matches results by function name, so remember id -> name
    let mut call_names: HashMap<String, String> = HashMap::new();

    for msg in messages.as_array().into_iter().flatten() {
        match msg["role"].as_str().unwrap_or("") {
            "system" => system_texts.push(text_of(&msg["content"])),

            "user" => {
                let text = text_of(&msg["content"]);
                if !text.is_empty() {
                    push_content(&mut contents, "user", vec![json!({ "text": text })]);
                }
            }

            "assistant" | "model" => {
                let mut parts = Vec::new();
                let text = text_of(&msg["content"]);
                if !text.is_empty() {
                    parts.push(json!({ "text": text }));
                }
                for call in msg["tool_calls"].as_array().into_iter().flatten() {
                    let name = call["function"]["name"].as_str().unwrap_or("").to_string();
                    if let Some(id) = call["id"].as_str() {
                        call_names.insert(id.to_string(), name.clone());
                    }
                    let args = match &call["function"]["arguments"] {
                        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| json!({})),
                        Value::Null => json!({}),
                        other => other.clone(),
                    };

                    let mut part = json!({ "functionCall": { "name": name, "args": args } });
                    match call["thought_signature"].as_str() {
                        Some(sig) => part["thoughtSignature"] = json!(sig),
                        // history saved before this fix has no signature (see below)
                        None => part["thoughtSignature"] = json!("skip_thought_signature_validator"),
                    }
                    parts.push(part);
                }
                push_content(&mut contents, "model", parts);
            }

            "tool" => {
                let id = msg["tool_call_id"].as_str().unwrap_or("");
                if !call_names.contains_key(id) {
                    continue; // no matching functionCall, skip
                }
                let name = msg["name"]
                    .as_str()
                    .map(String::from)
                    .or_else(|| call_names.get(id).cloned())
                    .unwrap_or_default();

                let raw = text_of(&msg["content"]);
                // `response` must be a JSON object
                let response = match serde_json::from_str::<Value>(&raw) {
                    Ok(v @ Value::Object(_)) => v,
                    Ok(v) => json!({ "result": v }),
                    Err(_) => json!({ "result": raw }),
                };
                push_content(
                    &mut contents,
                    "user",
                    vec![json!({ "functionResponse": { "name": name, "response": response } })],
                );
            }
            _ => {}
        }
    }

    let system = if system_texts.is_empty() {
        None
    } else {
        Some(json!({ "parts": [{ "text": system_texts.join("\n\n") }] }))
    };
    (system, contents)
}

pub fn apply_gemini_overrides(
    node_details: &Node,          // adjust to your type
    messages_value: &Value,              // adjust to your type
    envs: &mut HashMap<String, String>,
    body: &mut HashMap<String, Value>,
) {
    // model goes into the endpoint path: /v1beta/models/{model}:generateContent
    envs.insert("model".to_string(), node_details.model.clone());

    let (system, contents) = messages_to_gemini(messages_value);

    // Gemini expects an array here. Your original code stringified it;
    // see the note below.
    body.insert("contents".to_string(), json!(contents));

    match system {
        Some(sys) => { body.insert("systemInstruction".to_string(), json!(sys)); }
        None => { body.remove("systemInstruction"); }
    }

    let parsed_tools: Vec<Value> = node_details
        .parsed_tools
        .as_ref()
        .map(|strs| {
                strs.iter()
                    .filter_map(|s| serde_json::from_str::<Value>(s).ok())
                    .collect()
            })
            .unwrap_or_default();

    let parsed_tools = tools_to_gemini(parsed_tools);
        
    if parsed_tools.is_empty() {
        body.remove("tools"); // drop the unreplaced "tools" placeholder
    } else {
        body.insert("tools".to_string(), Value::Array(parsed_tools));
    }

    // Gemini doesn't use "messages" and rejects unreplaced placeholders
    for key in ["messages", "toolConfig", "safetySettings", "cachedContent"] {
        body.remove(key);
    }
}

fn clean_schema(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("additionalProperties");
            map.remove("$schema");
            for child in map.values_mut() {
                clean_schema(child);
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(clean_schema),
        _ => {}
    }
}

pub fn tools_to_gemini(parsed: Vec<Value>) -> Vec<Value> {
    let mut passthrough = Vec::new();
    let mut decls = Vec::new();

    for t in parsed {
        // already in Gemini format
        if t.get("functionDeclarations").is_some() {
            passthrough.push(t);
            continue;
        }
        // OpenAI format: {"type":"function","function":{...}}
        let f = t.get("function").cloned().unwrap_or(t);
        let mut decl = json!({
            "name": f.get("name").cloned().unwrap_or(json!("")),
            "description": f.get("description").cloned().unwrap_or(json!("")),
        });
        if let Some(p) = f.get("parameters") {
            let mut p = p.clone();
            clean_schema(&mut p);
            let no_props = p
                .get("properties")
                .and_then(|x| x.as_object())
                .map_or(true, |o| o.is_empty());
            if !no_props {
                decl["parameters"] = p;
            }
        }
        decls.push(decl);
    }

    let mut out = passthrough;
    if !decls.is_empty() {
        out.push(json!({ "functionDeclarations": decls }));
    }
    out
}

pub fn extract_message_gemini(json: &Value) -> Option<Message> {
    let parts = json
        .get("candidates")?
        .get(0)?
        .get("content")?
        .get("parts")?
        .as_array()?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);

    let mut text = String::new();
    let mut calls: Vec<Value> = Vec::new();

    for part in parts {
        // skip thought summaries if returned
        if part.get("thought").and_then(|t| t.as_bool()) == Some(true) {
            continue;
        }
        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
            text.push_str(t);
        }
        if let Some(fc) = part.get("functionCall") {
            let args = fc.get("args").cloned().unwrap_or_else(|| json!({}));
            let mut call = json!({
                "id": format!("{:08x}{}", nanos, calls.len() % 10),
                "type": "function",
                "function": {
                    "name": fc.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                    "arguments": args.to_string(),
                }
            });
            // the signature sits on the part, next to functionCall
            if let Some(sig) = part.get("thoughtSignature").and_then(|s| s.as_str()) {
                call["thought_signature"] = json!(sig);
            }
            calls.push(call);
        }
    }

    Some(Message {
        role: "assistant".to_string(),
        content: strip_thinking(&text),
        tool_call_id: None,
        name: None,
        tool_calls: if calls.is_empty() { None } else { Some(Value::Array(calls)) },
    })
}
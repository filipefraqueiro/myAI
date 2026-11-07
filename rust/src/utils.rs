use std::{
    collections::HashMap,
    fs::{
        File,
        create_dir,
    },
    pin::Pin,
    path::{PathBuf, Path},
    sync::Arc,
};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::{
    Value,
    json,
    Map,
    Number,
};
use chrono::Local;

// mine
use md_requests::md_requests::make_request;
use md_requests::md_requests::EndpointDetails;
use crate::gemini_utils::{
    messages_to_gemini,
    apply_gemini_overrides,
    extract_message_gemini,
};
use crate::python_tools_pyo3::execute_tool;
use crate::python_parser::{
    extract_function_schema,
    wrap_for_provider,
    // gemini_tools_payload,
};
use crate::structs::{
    DATA_FOLDER,
    Node,
    ModelProvider,
    Message,
    ToolCall,
};
#[cfg(not(feature = "cli"))]
use crate::api::flutter_stuff::{
    emit,
};
#[cfg(not(feature = "cli"))]
use crate::api::native_android_tools::{
    call_native,
    NATIVE_TOOLS,
    native_tool_schema,
};

pub fn extract_message(json: &Value) -> Option<Message> {
    let message = json
        .get("choices")?
        .get(0)?
        .get("message")?;

    let content = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    // keep tool_calls only if present and non-empty
    let tool_calls = message
        .get("tool_calls")
        .filter(|tc| tc.as_array().map_or(false, |a| !a.is_empty()))
        .cloned();

    Some(Message {
        role: message.get("role")?.as_str()?.to_string(),
        content: strip_thinking(content),
        tool_call_id: None,
        name: None,
        tool_calls,
    })
}

pub async fn extract_tool_calls(json: &Value) -> Option<Vec<ToolCall>> {
    let tool_calls_value = json
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("tool_calls")?;

    serde_json::from_value(tool_calls_value.clone()).ok()
}

pub async fn load_messages(node_name: &str) -> std::io::Result<Vec<Message>> {
    let file_path: String = format!("{}/{node_name}/messages.json", *DATA_FOLDER);

    let file = match OpenOptions::new().read(true).open(&file_path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => return Err(e),
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut messages = Vec::new();
    let mut line_num = 0;

    while let Some(line) = lines.next_line().await? {
        line_num += 1;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Message>(&line) {
            Ok(msg) => messages.push(msg),
            Err(e) => {
                eprintln!(
                    "[!] Skipping unparsable message at {}:{}: {e}",
                    file_path, line_num
                );
            }
        }
    }

    Ok(messages)
}

pub fn load_model_providers(file_path: String) -> Result<HashMap<String, ModelProvider>, String> {
    let file_path = PathBuf::from(file_path);
    let reader = File::open(file_path).map_err(|e| e.to_string())?;
    let model_providers: HashMap<String, ModelProvider> = serde_json::from_reader(reader).map_err(|e| e.to_string())?;
    // println!("{:#?}", model_providers);

    Ok(model_providers)
}

pub fn get_model_provider_details(model_providers:HashMap<String, ModelProvider>, model_provider: &str) -> Result<(EndpointDetails, Option<EndpointDetails>), String> {
    // get model provide details from the desired model_provider
    let model_provider_details: ModelProvider = model_providers
        .get(model_provider)
        .cloned()
        .ok_or_else(|| {
            format!("[!] Model Provider '{}' not found", model_provider)
        })?;
    // println!("{:#?}", model_provider_details);
    
    // get run_model endpoint details
    let run_model: EndpointDetails = model_provider_details.run_model;

    // get list_models endpoint details
    let list_models: Option<EndpointDetails> = model_provider_details.list_models;
    if list_models.is_none() {
        println!("[!] Provider '{}' does not have list_models", model_provider);
    }

    Ok((run_model, list_models))
}

pub fn load_nodes(file_path: String) -> Result<HashMap<String, Node>, String> {
    let file_path = PathBuf::from(file_path);
    let reader = File::open(file_path).map_err(|e| e.to_string())?;
    let nodes: HashMap<String, Node> = serde_json::from_reader(reader).map_err(|e| e.to_string())?;
    // println!("{:#?}", nodes);
    
    // create nodes folders if they do not exist
    for (node_key, _node) in &nodes {
        // println!("{:#?}", node_key);
        let folder_path: String = format!("{}/{node_key}", *DATA_FOLDER);
        let _ = create_dir(folder_path);
    }

    Ok(nodes)
}

pub fn get_node_details(nodes: HashMap<String, Node>, desired_node: &str) -> Result<Node, String> {
    let mut node: Node = nodes
        .get(desired_node)
        .cloned()
        .ok_or_else(|| {
            format!("[!] Node '{}' not found", desired_node)
        })?;
        // println!("{:#?}", node);
    
    
    if let Some(ref node_tools) = node.tools {
        // println!("{:#?}", node_tools);

        for (tool_name, tool) in node_tools {
            // println!("{:#?}", tool);

            let source = std::fs::read_to_string(&tool.path).map_err(|e| e.to_string())?;

            match extract_function_schema(&source, &tool_name) {
                Ok(schema) => {
                    // println!("{}", serde_json::to_string_pretty(&schema).unwrap());

                    match serde_json::to_string(&schema) {
                        Ok(_json_str) => {
                            let wrapped = wrap_for_provider(&tool_name, &schema, "openai");
                            node.parsed_tools.get_or_insert_with(Vec::new).push(wrapped.to_string());
                        }
                        Err(e) => {
                            eprintln!("[!] Failed to serialize schema for '{}': {e}", &tool_name);
                            // skip this tool, continue with the rest
                        }
                    }
                },
                Err(e) => eprintln!("[!] Introspection failed: {e}"),
            }
        }
    } 
    
    Ok(node)
}

pub fn strip_thinking(content: &str) -> String {
    let re = regex::Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    re.replace_all(content, "").trim().to_string()
}

pub async fn save_debug(new_message: &str, node_name: &str) -> std::io::Result<()> {
    let folder_path = format!("{}/{node_name}", *DATA_FOLDER);
    tokio::fs::create_dir_all(&folder_path).await?;

    let date = Local::now().format("%Y-%m-%d");
    let file_path = format!("{folder_path}/debug-{date}.json");

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .await?;

    file.write_all(new_message.as_bytes()).await?;
    file.write_all("\n".as_bytes()).await?;

    Ok(())
}

pub async fn save_message(new_message: &Message, node_name: &str) -> std::io::Result<()> {
    let file_path: String = format!("{}/{node_name}/messages.json", *DATA_FOLDER);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .await?;

    let mut json_line = serde_json::to_string(new_message)?;
    json_line.push('\n');

    file.write_all(json_line.as_bytes()).await?;

    Ok(())
}

// pub async fn save_message(new_message: &Message, node_name: &str) -> std::io::Result<()> {
//     tokio::fs::create_dir_all(node_name).await?;
//     let date = Local::now().format("%Y-%m-%d");
//     let file_path = format!("{node_name}/messages-{date}.json");

//     let mut file = OpenOptions::new()
//         .create(true)
//         .append(true)
//         .open(&file_path)
//         .await?;

//     let mut json_line = serde_json::to_string(new_message)?;
//     json_line.push('\n');

//     file.write_all(json_line.as_bytes()).await?;

//     Ok(())
// }

pub fn llm_call<'a>(run_model: EndpointDetails, mut envs: Arc<HashMap<String, String>>, debug: bool, node_name: &'a str, node_details: &'a Node, messages: &'a mut Vec<Message>) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send + 'a>> {
    // Make the HTTP request asynchronously
    Box::pin(async move {
        let mut run_model = run_model;
        let is_gemini = node_details.model_provider == "gemini";
        let mut pending_tool_calls: Option<Vec<ToolCall>> = None;

        // update the request body
        if let Ok(messages_value) = serde_json::to_value(&*messages) {
            // let body = run_model.body.get_or_insert_with(|| json!({}));
            let body = run_model.body.get_or_insert_with(HashMap::new);
        
            if is_gemini {
                apply_gemini_overrides(
                    &node_details,
                    &messages_value,
                    Arc::make_mut(&mut envs), // clones the map only if the Arc is shared
                    body,
                );
            }
            else {
                body.insert(
                    "messages".to_string(), 
                    json!(messages_value),
                );

                body.insert(
                    "model".to_string(),
                    serde_json::Value::String(node_details.model.clone()),
                );

                if let Some(strs) = &node_details.parsed_tools {
                    let parsed: Vec<serde_json::Value> = strs
                        .iter()
                        .filter_map(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                        .collect();

                    if !parsed.is_empty() {
      			        body.insert("tools".to_string(), Value::Array(parsed));
                    }
                }
            }

            let body = normalize_body(body);

            if debug {
                println!("body:\n{}", serde_json::to_string_pretty(&body)?);
            }
        }

        match make_request(run_model.clone(), Some(5), &envs, Some(debug)).await {
            Ok(json) => {
                let json_str = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
                let _ = save_debug(&json_str, node_name).await;

                if debug {
                    let json_str = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
                    println!("[+] {} DEBUG Response:\n{}", Local::now().format("%Y-%m-%d %H:%M:%S"), json_str);
                }

                let extracted = if is_gemini {
                    extract_message_gemini(&json)
                } else {
                    extract_message(&json)
                };

                if let Some(assistant_message) = extracted {
                    println!("\n\n");
                    println!("> {}", assistant_message.content);

                    // read tool calls before the message is moved into history
                    pending_tool_calls = if is_gemini {
                        tool_calls_from_message(&assistant_message)
                    } else {
                        extract_tool_calls(&json).await
                    };

                    messages.push(assistant_message.clone());
                    let _ = save_message(&assistant_message, node_name).await;
                }

                // run tools if the response has them
                if let Some(tool_calls) = pending_tool_calls {
                    // println!("tools: {:#?}", tool_calls);

                    // make tool calls
                    for call in &tool_calls {
                        let json_str = serde_json::to_string_pretty(&call).map_err(|e| e.to_string())?;
                        let _ = save_debug(&json_str, node_name).await;

                        if debug {
                            println!("[+] {} DEBUG tool call: {:#?}", Local::now().format("%Y-%m-%d %H:%M:%S"), call);
                        }

                        let tool = node_details.tools.as_ref().expect("REASON").get(&call.function.name)
                            .ok_or_else(|| format!("[+] {} ERROR unknown tool: {}", Local::now().format("%Y-%m-%d %H:%M:%S"), call.function.name))?;

                        let source = std::fs::read_to_string(&tool.path)
                            .map_err(|e| format!("[+] {} ERROR could not read {}: {e}", Local::now().format("%Y-%m-%d %H:%M:%S"), tool.path))?;
                        
                        let result = execute_tool(&source, &call.function.name, &call.function.arguments).await;
                        if debug {
                            println!("[+] {} DEBUG tool result: {:#?}", Local::now().format("%Y-%m-%d %H:%M:%S"), result);
                        }
                        
                        // Build the follow-up "tool" message to send back to the model
                        let tool_result_msg = Message {
                            role: String::from("tool"),
                            content: serde_json::to_string(&result).unwrap(),
                            tool_call_id: Some(call.id.clone()),
                            name: None,
                            tool_calls: None,
                        };

                        let json_str = serde_json::to_string_pretty(&tool_result_msg).map_err(|e| e.to_string())?;
                        let _ = save_debug(&json_str, node_name).await;

                        // append tool_result_msg to your `messages` array and send the next request
                        let _ = save_message(&tool_result_msg, node_name).await;
                        messages.push(tool_result_msg);
                    }

                    // tools ran -> send results back to the model, recurse
                    return llm_call(run_model, envs, debug, node_name, node_details, messages).await;
                }

                Ok(())
            }
            Err(e) => {
                eprintln!("[+] Error: {}", e);
                let error = serde_json::json!({ "error": e.to_string() });
                let json_str = serde_json::to_string_pretty(&error).map_err(|e| e.to_string())?;
                let _ = save_debug(&json_str, node_name).await;
                Ok(())
            }
        }
    })
}

pub fn tool_calls_from_message(m: &Message) -> Option<Vec<ToolCall>> {
    let v = m.tool_calls.as_ref()?;
    serde_json::from_value::<Vec<ToolCall>>(v.clone()).ok()
}

pub fn out(node: &str, kind: &str, text: impl Into<String>) {
    #[cfg(feature = "cli")]
    println!("[{node}] {kind}: {}", text.into());

    #[cfg(not(feature = "cli"))]
    emit(node, kind, text);
}

pub fn read_env_file(path: &Path) -> Result<HashMap<String, String>, dotenvy::Error> {
    dotenvy::from_path_iter(path)?.collect()
}

/// Top-level keys sent untouched (strings here must stay strings).
const RAW_KEYS: &[&str] = &["messages", "tools", "model", "stop"];

fn coerce_str(s: &str) -> Option<Value> {
    let t = s.trim();
    match t.to_ascii_lowercase().as_str() {
        "" | "none" | "null" => return None,          // remove
        "true" => return Some(Value::Bool(true)),
        "false" => return Some(Value::Bool(false)),
        _ => {}
    }
    if let Ok(i) = t.parse::<i64>() {
        return Some(Value::from(i));
    }
    if let Ok(f) = t.parse::<f64>() {
        if let Some(n) = Number::from_f64(f) {        // None for NaN/inf -> stays a string
            return Some(Value::Number(n));
        }
    }
    Some(Value::String(s.to_string()))
}

/// None = remove this entry.
fn coerce(v: Value) -> Option<Value> {
    match v {
        Value::Null => None,
        Value::String(s) => coerce_str(&s),
        Value::Array(a) => {
            let a: Vec<Value> = a.into_iter().filter_map(coerce).collect();
            (!a.is_empty()).then(|| Value::Array(a))
        }
        Value::Object(m) => {
            let m: Map<String, Value> =
                m.into_iter().filter_map(|(k, v)| coerce(v).map(|v| (k, v))).collect();
            (!m.is_empty()).then(|| Value::Object(m))
        }
        other => Some(other), // bool and number pass through (0 and false are kept)
    }
}

pub(crate) fn normalize_body(body: &mut HashMap<String, Value>) {
    let old = std::mem::take(body);
    *body = old
        .into_iter()
        .filter_map(|(k, v)| {
            if RAW_KEYS.contains(&k.as_str()) {
                Some((k, v))
            } else {
                coerce(v).map(|v| (k, v))
            }
        })
        .collect();
}


pub fn load_remote(node: &Node, providers_path: &str) -> Result<Option<(EndpointDetails, Option<EndpointDetails>)>, String> {
    if node.model_provider == "local" {
        return Ok(None);
    }
    let model_providers = load_model_providers(providers_path.to_string())
        .map_err(|e| e.to_string())?;
    let details = get_model_provider_details(model_providers, node.model_provider.as_str())
        .map_err(|e| e.to_string())?;

    Ok(Some(details))
}

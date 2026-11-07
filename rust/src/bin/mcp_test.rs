
use anyhow::{anyhow, Result, Context};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::RwLock;

// // mine
// use md_requests::md_requests::make_request;
// use md_requests::md_requests::EndpointDetails;
// use md_requests::md_requests::load_enpoints;
// use mcp_Stuff::McpClient;

#[tokio::main]
async fn main() -> Result<()> {
//     let mcp_url = "https://mcp.deepwiki.com/mcp";

//     let client = Arc::new(McpClient::new(mcp_url));

//         // Explicit scope to limit the file's lifetime
//     let endpoints = load_enpoints("mcp.json")?;

//     // if let Some(endpoint_details) = endpoints.get(&cli.endpoint) {
//     //     // println!("{:#?}", endpoint_details);
//     //     let output = make_request(endpoint_details.clone(), Some(5), &kargs, Some(debug)).await?;
//     //     println!("{:#}", output);
//     // } else {
//     //     println!("Endpoint {} not found", cli.endpoint);
//     // }  

//     let initialize_endpoint: EndpointDetails = endpoints.get("MCP_INITIALIZE")

//     let notification_endpoint = initialize_endpoint.clone();
//     let list_tools_endpoint = initialize_endpoint.clone();
//     let call_tool_endpoint = initialize_endpoint.clone();

//     // Initialize MCP connection
//     println!("Initializing MCP connection...");

//     let initialization = client.initialize(
//         initialize_endpoint,
//         notification_endpoint,
//         Some(60),
//         Some(true),
//     ).await?;

//     println!("MCP initialized: {}", initialization);

//     // Discover MCP tools
//     println!("Discovering MCP tools...");

//     let mcp_tools = client.list_tools(
//         list_tools_endpoint,
//         Some(60),
//         Some(false),
//     ).await?;

//     println!("Discovered {} tools", mcp_tools.len());

//     for tool in &mcp_tools {
//         println!(
//             "Tool: {}",
//             tool["name"].as_str().unwrap_or("unknown")
//         );
//     }


//     // Register MCP tools in unified registry
//     // let mut registry = ToolRegistry::new();

//     // registry.register_mcp_tools(
//     //     "deepwiki",
//     //     client.clone(),
//     //     call_tool_endpoint,
//     //     mcp_tools,
//     // );

//     // // These are the function definitions sent to the LLM.
//     // let llm_tools = registry.get_definitions();

//     // println!("Available LLM functions:");
//     // println!("{}", serde_json::to_string_pretty(&llm_tools)?);


//     // // Send functions to LLM
//     // let messages = vec![
//     //     json!({
//     //         "role": "user",
//     //         "content": "Explain the architecture of the tokio-rs/axum repository."
//     //     })
//     // ];

//     // Provider-specific implementation.
//     // Send messages and llm_tools to Mistral, OpenAI, etc.
//     //
//     // let llm_response = call_llm(
//     //     messages,
//     //     llm_tools
//     // ).await?;

//     // --------------------------------------------------
//     // 6. Example LLM tool call
//     // --------------------------------------------------

//     // Simulate the function call returned by the LLM.
//     // The LLM would normally generate this itself.

//     // let function_name = "deepwiki__ask_wiki_question";

//     // let arguments = json!({
//     //     "repoName": "tokio-rs/axum",
//     //     "question": "Explain the architecture and main components of this repository."
//     // });

//     // println!("Executing LLM function: {}", function_name);

//     // let result = registry.execute(
//     //     function_name,
//     //     arguments,
//     //     Some(120),
//     //     Some(true),
//     // ).await?;

//     // println!("MCP tool result:");
//     // println!("{}", serde_json::to_string_pretty(&result)?);

//     // --------------------------------------------------
//     // 7. Return result to LLM
//     // --------------------------------------------------

//     // Append the tool result to the conversation.
//     //
//     // messages.push(json!({
//     //     "role": "assistant",
//     //     "tool_calls": [...]
//     // }));
//     //
//     // messages.push(json!({
//     //     "role": "tool",
//     //     "tool_call_id": "...",
//     //     "content": result.to_string()
//     // }));
//     //
//     // Call the LLM again to generate the final answer.
//     //
//     // let final_response = call_llm(
//     //     messages,
//     //     llm_tools
//     // ).await?;

//     println!("MCP execution completed.");

    Ok(())
}

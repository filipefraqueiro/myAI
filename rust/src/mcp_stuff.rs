
// use anyhow::{anyhow, Result, Context};
// use serde_json::{json, Value};
// use std::collections::HashMap;
// use std::sync::{
//     atomic::{AtomicU64, Ordering},
//     Arc,
// };
// use tokio::sync::RwLock;

// // mine
// use md_requests::md_requests::make_request;
// use md_requests::md_requests::EndpointDetails;
// use md_requests::md_requests::load_enpoints;

// const PROTOCOL_VERSION: &str = "2025-11-25";

// #[derive(Clone)]
// pub struct McpClient {
//     url: String,
//     protocol_version: Arc<RwLock<String>>,
//     session_id: Arc<RwLock<Option<String>>>,
//     request_id: Arc<AtomicU64>,
// }

// impl McpClient {
//     /// Create an MCP client using only the server URL.
//     pub fn new(url: impl Into<String>) -> Self {
//         Self {
//             url: url.into(),
//             session_id: Arc::new(RwLock::new(None)),
//             protocol_version: Arc::new(
//                 RwLock::new(PROTOCOL_VERSION.to_string())
//             ),
//             request_id: Arc::new(AtomicU64::new(1)),
//         }
//     }

//     fn next_id(&self) -> u64 {
//         self.request_id.fetch_add(1, Ordering::Relaxed)
//     }

//     /// Execute an MCP request through the existing make_request function.
//     async fn execute(
//         &self,
//         endpoint_details: EndpointDetails,
//         body: Value,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         let mut kargs = HashMap::new();

//         kargs.insert(
//             "MCP_SERVER_URL".to_string(),
//             self.url.clone()
//         );

//         kargs.insert(
//             "MCP_REQUEST_BODY".to_string(),
//             serde_json::to_string(&body)?
//         );

//         kargs.insert(
//             "MCP_PROTOCOL_VERSION".to_string(),
//             self.protocol_version.read().await.clone()
//         );

//         // Include existing session ID
//         if let Some(session_id) = self.session_id.read().await.as_ref() {
//             kargs.insert(
//                 "MCP_SESSION_ID".to_string(),
//                 session_id.clone()
//             );
//         }

//         let response = make_request(
//             endpoint_details,
//             timeout,
//             &kargs,
//             debug,
//         ).await?;

//         // Capture session ID returned by server
//         if let Some(session_id) = response.headers
//             .get("mcp-session-id")
//         {
//             *self.session_id.write().await =
//                 Some(session_id.clone());
//         }

//         Ok(response.body)
//     }

//     /// Execute a JSON-RPC request.
//     pub async fn request(
//         &self,
//         endpoint_details: EndpointDetails,
//         method: &str,
//         params: Value,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         let id = self.next_id();

//         let body = json!({
//             "jsonrpc": "2.0",
//             "id": id,
//             "method": method,
//             "params": params
//         });

//         let response = self.execute(
//             endpoint_details,
//             body,
//             None,
//             timeout,
//             debug,
//         ).await?;

//         // Check JSON-RPC errors
//         if let Some(error) = response.get("error") {
//             return Err(anyhow!(
//                 "MCP JSON-RPC error: {}",
//                 error
//             ));
//         }

//         // Validate response ID
//         if response.get("id").and_then(Value::as_u64) != Some(id) {
//             return Err(anyhow!(
//                 "MCP response ID does not match request ID"
//             ));
//         }

//         // Return JSON-RPC result
//         response.get("result")
//             .cloned()
//             .ok_or_else(|| anyhow!("Missing MCP result"))
//     }

//     /// Execute an MCP notification (no JSON-RPC ID).
//     pub async fn notification(
//         &self,
//         endpoint_details: EndpointDetails,
//         method: &str,
//         params: Value,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<()> {

//         let body = json!({
//             "jsonrpc": "2.0",
//             "method": method,
//             "params": params
//         });

//         let response = self.execute(
//             endpoint_details,
//             body,
//             None,
//             timeout,
//             debug,
//         ).await?;

//         // Notifications should not return a JSON-RPC result.
//         if !response.is_null() {
//             return Err(anyhow!(
//                 "Expected empty notification response"
//             ));
//         }

//         Ok(())
//     }

//     /// Initialize the MCP connection.
//     pub async fn initialize(
//         &self,
//         endpoint_details: EndpointDetails,
//         notification_endpoint: EndpointDetails,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         let result = self.request(
//             endpoint_details,
//             "initialize",
//             json!({
//                 "protocolVersion": PROTOCOL_VERSION,
//                 "capabilities": {},
//                 "clientInfo": {
//                     "name": "the360",
//                     "version": "1.0.0"
//                 }
//             }),
//             timeout,
//             debug,
//         ).await?;

//         // Read negotiated protocol version
//         let negotiated = result["protocolVersion"]
//             .as_str()
//             .ok_or_else(|| anyhow!(
//                 "Server did not return a protocol version"
//             ))?;

//         *self.protocol_version.write().await =
//             negotiated.to_string();

//         // Notify server that initialization is complete
//         self.notification(
//             notification_endpoint,
//             "notifications/initialized",
//             json!({}),
//             timeout,
//             debug,
//         ).await?;

//         Ok(result)
//     }

//     /// Retrieve all available MCP tools (including pagination).
//     pub async fn list_tools(
//         &self,
//         endpoint_details: EndpointDetails,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Vec<Value>> {

//         let mut tools = Vec::new();
//         let mut cursor: Option<String> = None;

//         loop {
//             let mut params = json!({});

//             if let Some(value) = &cursor {
//                 params["cursor"] = json!(value);
//             }

//             let result = self.request(
//                 endpoint_details.clone(),
//                 "tools/list",
//                 params,
//                 timeout,
//                 debug,
//             ).await?;

//             if let Some(items) = result["tools"].as_array() {
//                 tools.extend(items.iter().cloned());
//             }

//             cursor = result["nextCursor"]
//                 .as_str()
//                 .map(str::to_string);

//             if cursor.is_none() {
//                 break;
//             }
//         }

//         Ok(tools)
//     }

//     /// Execute an MCP tool.
//     pub async fn call_tool(
//         &self,
//         endpoint_details: EndpointDetails,
//         name: &str,
//         arguments: Value,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         self.request(
//             endpoint_details,
//             "tools/call",
//             json!({
//                 "name": name,
//                 "arguments": arguments
//             }),
//             timeout,
//             debug,
//         ).await
//     }

//     /// Retrieve available MCP resources.
//     pub async fn list_resources(
//         &self,
//         endpoint_details: EndpointDetails,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Vec<Value>> {

//         let result = self.request(
//             endpoint_details,
//             "resources/list",
//             json!({}),
//             timeout,
//             debug,
//         ).await?;

//         Ok(result["resources"]
//             .as_array()
//             .cloned()
//             .unwrap_or_default())
//     }

//     /// Read an MCP resource.
//     pub async fn read_resource(
//         &self,
//         endpoint_details: EndpointDetails,
//         uri: &str,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         self.request(
//             endpoint_details,
//             "resources/read",
//             json!({ "uri": uri }),
//             timeout,
//             debug,
//         ).await
//     }

//     /// Retrieve available MCP prompts.
//     pub async fn list_prompts(
//         &self,
//         endpoint_details: EndpointDetails,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Vec<Value>> {

//         let result = self.request(
//             endpoint_details,
//             "prompts/list",
//             json!({}),
//             timeout,
//             debug,
//         ).await?;

//         Ok(result["prompts"]
//             .as_array()
//             .cloned()
//             .unwrap_or_default())
//     }

//     /// Retrieve a specific MCP prompt.
//     pub async fn get_prompt(
//         &self,
//         endpoint_details: EndpointDetails,
//         name: &str,
//         arguments: Value,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         self.request(
//             endpoint_details,
//             "prompts/get",
//             json!({
//                 "name": name,
//                 "arguments": arguments
//             }),
//             timeout,
//             debug,
//         ).await
//     }

//     /// MCP ping.
//     pub async fn ping(
//         &self,
//         endpoint_details: EndpointDetails,
//         timeout: Option<u64>,
//         debug: Option<bool>,
//     ) -> Result<Value> {

//         self.request(
//             endpoint_details,
//             "ping",
//             json!({}),
//             timeout,
//             debug,
//         ).await
//     }
// }
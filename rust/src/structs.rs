use serde::{
    Deserialize,
    Serialize
};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::LazyLock,
};
#[cfg(feature = "cli")]
use clap::Parser;

// mine
use md_requests::md_requests::EndpointDetails;

pub static DATA_FOLDER: LazyLock<String> = LazyLock::new(|| {
    "data".to_string()
});

// Command line arguments
#[cfg(feature = "cli")]
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    // Path to the .env file
    #[arg(long, default_value = ".env")]
    pub env_file: PathBuf,
    // Path to the endpoints JSON file
    #[arg(short, long, default_value = "nodes.json")]
    pub nodes_file: PathBuf,
    // Endpoint name to call
    #[arg(short, long, default_value = "model_providers.json")]
    pub model_providers_file: PathBuf,
}

#[cfg(feature = "cli")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePrompt {
    pub prompt_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub model_provider: String,
    pub model: String,
    pub prompts: Option<Vec<NodePrompt>>,
    pub tools: Option<HashMap<String, NodeTool>>,
    // pub parsed_tools: Option<Vec<Value>>,
    pub parsed_tools: Option<Vec<String>>,
    // output_format: String,
    // available_next_nodes: String,
    pub are_next_nodes_conditional: Option<bool>,
    pub next_nodes_condition: Option<String>,
    pub next_nodes_parallel: Option<bool>,
    pub store: Option<bool>,
    pub use_store: Option<bool>,
    pub human_in_the_loop: Option<bool>,
    pub schedules: Option<Vec<Schedule>>,
}

// #[derive(Clone, Serialize, Deserialize, Debug)]
// pub struct NodePrompt {
//     pub path: String,
// }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Schedule {
    pub prompt: String,
    pub interval: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeTool {
    pub path: String,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tool_calls: Option<Value>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ModelProvider {
    pub run_model: EndpointDetails,
    pub list_models: Option<EndpointDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug)]
pub enum NodeMessage {
    // Focus,
    // Unfocus,
    Shutdown,
    ViewNode,
    ViewNodeProvider,
    ListProviderModels,
    DebugOn,
    DebugOff,
    ClearMessages,
    Reload,
    Input(String),
}

// #[derive(Debug, Deserialize)]
// struct AssistantMessage {
//     role: String,
//     #[serde(default)]
//     tool_calls: Vec<ToolCall>,
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct usage_stats {

// }

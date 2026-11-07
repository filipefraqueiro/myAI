use std::{
    collections::HashMap,
    sync::Arc,
    path::Path,
};
use tokio::sync::mpsc;
use clap::Parser;

// mine local
mod python_parser;
mod python_tools;
mod python_tools_pyo3;
mod gemini_utils;
mod utils;
mod structs;
mod brain;
mod cmd_line;
mod secure;
mod local_llm;

use utils::{
    load_nodes,
    read_env_file,
};
use structs::{
    Cli,
    NodeMessage,
};
use brain::run_node_loop;
use cmd_line::run_stdin_router;
use local_llm::LlmState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // get cmd line options
    let cli = Arc::new(Cli::parse());
    // println!("{:#?}", cli);

    // read .env
    let envs = Arc::new(read_env_file(Path::new(&cli.env_file)).unwrap_or_else(|e| {
        eprintln!("[+] Warning: failed to load env file: {e}");
        HashMap::new()
    }));
    // println!("{:#?}", envs.keys());
    
    let model_providers_file_path = cli.model_providers_file.display().to_string();
    let nodes_file_path = cli.nodes_file.display().to_string();

    // load the nodes
    let nodes = load_nodes(nodes_file_path.clone()).map_err(|e| e.to_string())?;
    let node_names: Vec<String> = nodes.keys().cloned().collect();

    let mut node_senders = HashMap::new();

    let llm = LlmState::default();

    for (node_name, _node_details) in nodes {
        let (tx, rx) = mpsc::channel::<NodeMessage>(32);
        node_senders.insert(node_name.clone(), tx);

        let envs = Arc::clone(&envs);
        let nodes_file_path_ = nodes_file_path.clone();
        let model_providers_file_path_ = model_providers_file_path.clone();
        let llm = llm.clone();

        // Spawn an async task on the Tokio runtime for each node
        tokio::spawn(async move {
            if let Err(e) = run_node_loop(model_providers_file_path_, nodes_file_path_, &node_name, envs.clone(), rx, llm).await {
                eprintln!("[+] Error in node {}: {:?}", node_name, e);
            }
        });
    }

    run_stdin_router(node_names, node_senders, envs.clone(), llm.clone()).await?;

    Ok(())
}


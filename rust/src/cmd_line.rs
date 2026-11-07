use std::{
    collections::HashMap,
    sync::{
        atomic::Ordering,
        Arc
    },
    process,
    path::{
        Path,
        PathBuf,
    },
};
use tokio::{
    sync::mpsc,
};
use rustyline::DefaultEditor;

// mine local
use crate::structs::{
    NodeMessage,
};
use crate::local_llm::{
    download_model,
    Engine,
    LlmState,
};

// Central Stdin Router reading line by line
pub async fn run_stdin_router(nodes: Vec<String>, mut senders: HashMap<String, mpsc::Sender<NodeMessage>>, envs: Arc<HashMap<String, String>>, mut llm: LlmState) -> Result<(), Box<dyn std::error::Error>> {
    println!("[+] PID is {}", process::id());

    // Default current node to the first node
    let mut current_node = nodes[0].clone();

    println!("[+] Use `/help` to see the available commands.");
    println!("[+] Available nodes: {:?}", nodes);
    println!("[+] Current node: {:?}\n", current_node);

    let mut rl_rx = spawn_rustyline_bridge();

    while let Some(line) = rl_rx.recv().await {
        // trim message to avoid white spaces
        let trimmed = line.trim();

        // if input empty ignore
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        match parts[0] {
            "/help" => {
                println!("\n[+] Management Commands:");
                println!("[+] Global:\n");
                println!("  /help                                       - Print usage");
                println!("  /switch <NodeName>                          - Focus to specific node");
                println!("  /list-nodes                                 - Show node inventory");
                println!("  /envs                                       - Print ENV variables");
                println!("  /start-local-llm <file.gguf>                - Starts the local llm");
                println!("  /stop-local-llm                             - Stops the local llm");
                println!("  /download-local-model <huggingface_url>     - Downloads a model to run localy");
                println!("\n[+] Node:\n");
                println!("  /view-node              - Print node details");
                println!("  /view-node-provider     - View the node model provider details");
                println!("  /list-provider-models   - List the node provider models");
                println!("  /debug <on,off>         - Turns the debug on/off");
                println!("  /clear-msgs             - Clear the messages history (only for this session)");
                println!("  /reload                 - Reload node details");
                println!("  /exit                   - Exit");
                println!();
            }
            "/switch" => {
                if parts.len() == 2 {
                    let target_node = parts[1].to_string();
                    if senders.contains_key(&target_node) {
                        current_node = target_node;
                    } else {
                        println!("Unknown node '{}'. Available: {:?}", target_node, nodes);
                    }
                } else {
                    println!("[+] Usage: /switch <NodeName>");
                }
            }
            "/list-nodes" => {
                println!("\n[+] Registered Nodes:");
                for node in &nodes {
                    let active_marker = if node == &current_node { " (focused)" } else { "" };
                    println!(" - {}{}", node, active_marker);
                }
                println!();
            }
            "/envs" => {
                println!("{:#?}", envs);
            }
            "/start-local-llm" => match parts.get(1).and_then(|n| Path::new(n).file_name()) {
                None => println!("[!] usage: /start-local-llm <file.gguf>"),
                Some(name) => {
                    // free any previously loaded model first
                    *llm.engine.write().unwrap() = None;
                    
                    let path = PathBuf::from("models").join(name);

                    println!("[+] Starting Local LLM: {}", path.display());
                    
                    match tokio::task::spawn_blocking(move || Engine::load(&path)).await {
                        Ok(Ok(e)) => {
                            *llm.engine.write().unwrap() = Some(Arc::new(e));
                            println!("[+] Local LLM ready");
                        }
                        Ok(Err(e)) => println!("[!] load failed: {e:#}"),
                        Err(e) => println!("[!] load task failed: {e}"),
                    }
                }
            },
            "/stop-local-llm" => {
                llm.cancel.store(true, Ordering::SeqCst); // interrupts generation
                // model is freed once the last Arc drops
                *llm.engine.write().unwrap() = None;
                println!("[+] Stopping Local LLM");
            }
            "/download-local-model" => {
                if parts.len() == 2 {
                    let url:String = parts[1].to_string();
                    println!("[+] Dowloading model: {:?}", url);
                    
                    let models_dir = PathBuf::from("models");

                    let path = download_model(&url, &models_dir, |done, total| match total {
                        Some(t) => eprint!("\rdownloading: {:.1}%", done as f64 / t as f64 * 100.0),
                        None => eprint!("\rdownloading: {} MB", done / 1_000_000),
                    })
                    .await?;
                    eprintln!("\nmodel: {}", path.display());
                }
            }
            "/view-node" => {
                senders[&current_node].send(NodeMessage::ViewNode).await.ok();
            }
            "/view-node-provider" => {
                senders[&current_node].send(NodeMessage::ViewNodeProvider).await.ok();
            }
            "/list-provider-models" => {
                senders[&current_node].send(NodeMessage::ListProviderModels).await.ok();
            }
            "/debug" => {
                if let Some(&"on") = parts.get(1) {
                    senders[&current_node].send(NodeMessage::DebugOn).await.ok();
                }
                else {
                    senders[&current_node].send(NodeMessage::DebugOff).await.ok();
                }
            }
            "/clear-msgs" => {
                senders[&current_node].send(NodeMessage::ClearMessages).await.ok();
            }
            "/reload" => {
                senders[&current_node].send(NodeMessage::Reload).await.ok();
            }
            "/exit" => {
                println!("\n[+] Shutting down all node tasks...");

                // Broadcast Shutdown signal to all running nodes
                for (name, tx) in senders.drain() {
                    let _ = tx.send(NodeMessage::Shutdown).await;
                    println!("[+] Sent shutdown signal to node '{}'.", name);
                }

                // Yield briefly to let background threads execute their cleanup prints
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                println!("[+] All threads stopped. Exiting system.\n");
                break; // Exit router loop
            }
            _ => {
                // Forward input to the active node task
                senders[&current_node].send(NodeMessage::Input(trimmed.to_string())).await.ok();
            }
        }

        // Small yield to let async node print output before prompt redraws
        tokio::task::yield_now().await;
    }

    Ok(())
}

fn spawn_rustyline_bridge() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>(32);

    std::thread::spawn(move || {
        let mut rl = match DefaultEditor::new() {
            Ok(rl) => rl,
            Err(e) => {
                eprintln!("[+] Failed to init rustyline: {e}");
                return;
            }
        };

        loop {
            match rl.readline("> ") {
                Ok(line) => {
                    let _ = rl.add_history_entry(line.as_str());
                    if tx.blocking_send(line).is_err() {
                        break; // receiver dropped, router shut down
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted)
                | Err(rustyline::error::ReadlineError::Eof) => break,
                Err(e) => {
                    eprintln!("[+] Readline error: {e}");
                    break;
                }
            }
        }
    });

    rx
}
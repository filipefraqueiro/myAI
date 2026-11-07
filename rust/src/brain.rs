use std::{
    collections::HashMap,
    sync::Arc,
    env,
};
use tokio::sync::mpsc;
use tokio_cron_scheduler::{
    JobScheduler,
    Job
};
use anyhow::Result;

// mine
use md_requests::md_requests::{
    make_request,
    EndpointDetails,
};

// mine local
use crate::utils::{
    save_message,
    load_messages,
    load_model_providers,
    get_model_provider_details,
    load_nodes,
    get_node_details,
    save_debug,
    llm_call,
    load_remote,
};
use crate::structs::{
    Node,
    Message,
    NodeMessage,
};
use crate::secure::check_for_secrets;
use crate::local_llm::{
    LlmState,
    local_llm_call,
};

pub async fn run_node_loop(model_providers_file_path: String, nodes_file_path: String, node_name: &str, mut envs: Arc<HashMap<String, String>>, mut rx: mpsc::Receiver<NodeMessage>, llm: LlmState) -> Result<(), Box<dyn std::error::Error>> {
    let mut messages: Vec<Message> = load_messages(node_name).await.unwrap_or_else(|e| {
        eprintln!("[+] Failed to load messages for '{}': {e}", node_name);
        Vec::new()
    });

    let mut message: Message;
    let mut debug: bool = false;

    let mut nodes = load_nodes(nodes_file_path.clone()).map_err(|e| e.to_string())?;
    // println!("{:#?}", node_details.model);

    let mut node_details: Node = get_node_details(nodes, &node_name).map_err(|e| e.to_string())?;
    // println!("{:#?}", node_details.model_provider);
    // println!("{:#?}", node_details.model_provider);

    // let json_str = serde_json::to_string_pretty(&node_details).map_err(|e| e.to_string())?;
    // let _ = save_debug(&json_str, node_name).await;

    let mut remote = load_remote(&node_details, &model_providers_file_path)?;

    // start the node schedules
    // let sched = JobScheduler::new().await?;
    let sched = JobScheduler::new().await.map_err(|e| e.to_string())?;

    if let Some(schedules) = node_details.schedules.clone() {
        for schedule in schedules {
            if schedule.prompt.trim().is_empty() {
                continue;
            }

            if schedule.interval.trim().is_empty() {
                continue;
            }

            // Cheaply clone Arc references and owned data for the job
            let envs = Arc::clone(&envs);
            let llm = llm.clone();
            let node_name = node_name.to_string();
            let node_details = node_details.clone();
            let run_model = remote.as_ref().map(|(r, _)| r.clone());
            let prompt = schedule.prompt.clone();

            let job = Job::new_async(schedule.interval.as_str(), move |_uuid, _l| {
                // Clone per execution
                let envs = Arc::clone(&envs);
                let llm = llm.clone();
                let node_name = node_name.clone();
                let node_details = node_details.clone();
                let run_model = run_model.clone();
                let prompt = prompt.clone();    

                Box::pin(async move {
                    let mut schedule_messages = vec![Message {
                        role: "user".into(),
                        content: prompt,
                        tool_call_id: None,
                        name: None,
                        tool_calls: None,
                    }];

                    // Call async llm_call with owned references
                    let res: Result<(), String> = match run_model {
                        Some(run_model) => {
                            llm_call(run_model, envs, debug, &node_name, &node_details, &mut schedule_messages)
                                .await
                                .map(|_| ())
                                .map_err(|e| format!("{e:?}"))
                        }
                        None => {
                            local_llm_call(llm, envs, debug, &node_name, &node_details, &mut schedule_messages)
                                .await
                                .map(|_| ())
                                .map_err(|e| format!("{e:#}"))
                        }
                    };

                    if let Err(e) = res {
                        eprintln!("[!] Scheduled llm call error for node {node_name}: {e}");
                    }
                })
            })?;

            sched.start().await.map_err(|e| e.to_string())?;
        }
    }

    // Start scheduler in background so run node loop continues non-blocking
    sched.start().await.map_err(|e| e.to_string())?;

    // run node forever
    while let Some(msg) = rx.recv().await {
        match msg {
            NodeMessage::Shutdown => {
                println!("[+] [{}] Running cleanup tasks...", node_name);
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                println!("[+] [{}] Terminated.", node_name);
                break; // Exit task loop
            }
            NodeMessage::ViewNode => {
                println!("{:#?}", node_details);
            }
            NodeMessage::ViewNodeProvider => {
                match &remote {
                    Some((run_model, _list_models)) => {
                        println!("{:#?}", run_model);
                    }
                    None => {
                        println!("[+] This is a local node");
                    }
                }
            }
            NodeMessage::ListProviderModels => {
                match &remote {
                    Some((_run_model, list_models)) => {
                        match &list_models {
                            Some(lm) => {
                                match make_request(lm.clone(), Some(5), &envs, None).await {
                                    Ok(json) => println!("{}", serde_json::to_string_pretty(&json)?),
                                    Err(e) => eprintln!("[!] Error: {}", e),
                                }
                            }
                            None => {
                                eprintln!("[+] Provider '{}' does not support list_models.", &node_name);
                            }
                        }
                    }
                    None => {
                        println!("[+] This is a local node");
                    }
                }


            }
            NodeMessage::DebugOn => {
                debug = true;
            }
            NodeMessage::DebugOff => {
                debug = false;
            }
            NodeMessage::ClearMessages => {
                messages = Vec::new();
            }
            NodeMessage::Reload => {
                // read .env
                dotenvy::dotenv().map_err(|e| e.to_string())?;
                envs = Arc::new(env::vars().collect::<HashMap<String, String>>());

                nodes = load_nodes(nodes_file_path.clone()).map_err(|e| e.to_string())?;
                node_details = get_node_details(nodes, &node_name).map_err(|e| e.to_string())?;
                
                remote = load_remote(&node_details, &model_providers_file_path)?;

                messages = load_messages(node_name).await.unwrap_or_else(|e| {
                    eprintln!("[!] Failed to load messages for '{}': {e}", node_name);
                    Vec::new()
                });
            }
            NodeMessage::Input(input) => {
                match check_for_secrets(input.clone(), envs.clone()) {
                    Ok(secure_input) => {
                        // create message struct
                        message = Message {
                            role: String::from("user"),
                            content: secure_input,
                            tool_call_id: None,
                            name: None,
                            tool_calls: None
                        };

                        // save message
                        let _ = save_message(&message, node_name).await;
                        let json_str = serde_json::to_string_pretty(&message).map_err(|e| e.to_string())?;
                        let _ = save_debug(&json_str, node_name).await;
                        
                        // add message to the messages Vec
                        messages.push(message);

                        // make the llm call
                        let result: Result<(), String> = match &remote {
                            Some((run_model, _)) => llm_call(run_model.clone(), envs.clone(), debug, node_name, &node_details, &mut messages)
                                .await
                                .map(|_| ())
                                .map_err(|e| e.to_string()),
                            None => local_llm_call(llm.clone(), envs.clone(), debug, node_name, &node_details, &mut messages)
                                .await
                                .map(|_| ())
                                .map_err(|e| format!("{e:#}")),
                        };

                        if let Err(e) = result {
                            eprintln!("Error: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
        }
    }

    Ok(())
}

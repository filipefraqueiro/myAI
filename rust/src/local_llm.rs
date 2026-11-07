use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, LlamaChatMessage, LlamaModel},
    sampling::LlamaSampler,
    send_logs_to_tracing, LogOptions
};
use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Once,
    io::Write,
    collections::HashMap,
};
use tokio::{fs, io::AsyncWriteExt};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use serde_json::Value;

// mine
use crate::structs::{
    Node,
    Message,
};
use crate::utils::{
    save_message,
    save_debug,
};

static LOGS: Once = Once::new();

#[derive(Default, Clone)]
pub struct LlmState {
    pub engine: Arc<std::sync::RwLock<Option<Arc<Engine>>>>,
    pub cancel: Arc<AtomicBool>,
    pub busy: Arc<AtomicBool>,   // one generation at a time
}

/// Download a .gguf file into `dir`. Returns the final path.
/// Skips the download if the file already exists. Writes to `<name>.part`
/// first so an interrupted download never looks like a valid model.
pub async fn download_model(
    url: &str,
    dir: &Path,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<PathBuf> {
    let name = url
        .split('?')
        .next()
        .and_then(|u| u.rsplit('/').next())
        .filter(|n| n.ends_with(".gguf"))
        .context("URL must end in a .gguf filename")?;

    fs::create_dir_all(dir).await?;
    let dest = dir.join(name);
    if dest.exists() {
        return Ok(dest);
    }
    let part = dir.join(format!("{name}.part"));

    let resp = reqwest::get(url).await?.error_for_status()?;
    let total = resp.content_length();
    let mut file = fs::File::create(&part).await?;
    let mut done = 0u64;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;
        on_progress(done, total);
    }
    file.flush().await?;
    drop(file);
    fs::rename(&part, &dest).await?;
    Ok(dest)
}

// With repeated start and stop cycles on a phone, drop order matters. Rust drops fields in declaration order, and your Engine has backend first. Swap them so the model is freed before the backend:
pub struct Engine {
    model: LlamaModel,
    backend: LlamaBackend,
}

impl Engine {
    pub fn load(model_path: &Path) -> Result<Self> {
        LOGS.call_once(|| send_logs_to_tracing(LogOptions::default().with_logs_enabled(false)));

        let backend = LlamaBackend::init()?;
        // Add `.with_n_gpu_layers(n)` here when built with a GPU feature.
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(&backend, model_path, &params)
            .context("failed to load model")?;
        Ok(Self { backend, model })
    }

    /// Single-turn chat. Calls `on_token` for each decoded text fragment.
    /// Blocking: run it via `spawn_blocking` from async code or a worker thread.
    pub fn generate(
        &self,
        messages: &[(String, String)],
        max_new_tokens: i32,
        cancel: &AtomicBool,
        mut on_token: impl FnMut(&str),
    ) -> Result<String> {
        const N_CTX: u32 = 2048;

        // Apply the model's built-in chat template.
        let tmpl = self.model.chat_template(None)?;
        let msgs = messages
            .iter()
            .map(|(r, c)| LlamaChatMessage::new(r.clone(), c.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let prompt = self.model.apply_chat_template(&tmpl, &msgs, true)?;

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_threads(4); // tune per device
        let mut ctx = self.model.new_context(&self.backend, ctx_params)?;

        // The chat template already contains BOS/special markers as text, so
        // don't add them again; parse_special=true turns them into real tokens.
        let vocab = self.model.vocab();
        let tokens = vocab.tokenize(prompt.as_bytes(), false, true);
        if tokens.len() as u32 + max_new_tokens as u32 > N_CTX {
            bail!("prompt + max_new_tokens exceeds context size ({N_CTX})");
        }

        // Feed the prompt; only the last token needs logits.
        let mut batch = LlamaBatch::new(tokens.len().max(512), 1);
        let last = tokens.len() as i32 - 1;
        for (i, t) in tokens.iter().enumerate() {
            batch.add(*t, i as i32, &[0], i as i32 == last)?;
        }
        ctx.decode(&mut batch)?;

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::top_k(40),
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::temp(0.7),
            LlamaSampler::dist(1234),
        ]);

        let mut out = String::new();
        let mut pending: Vec<u8> = Vec::new(); // tokens can split UTF-8 chars
        let mut n_cur = batch.n_tokens();
        let mut n_gen = 0;

        while n_gen < max_new_tokens && !cancel.load(Ordering::Relaxed) {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);
            if vocab.is_eog(token) {
                break;
            }

            pending.extend(vocab.token_to_piece(token, false, None));
            match std::str::from_utf8(&pending) {
                Ok(s) => {
                    on_token(s);
                    out.push_str(s);
                    pending.clear();
                }
                Err(e) if e.error_len().is_none() => {
                    // incomplete char at the end: emit the valid prefix, keep the rest
                    let n = e.valid_up_to();
                    if n > 0 {
                        let s = std::str::from_utf8(&pending[..n]).unwrap().to_owned();
                        on_token(&s);
                        out.push_str(&s);
                        pending.drain(..n);
                    }
                }
                Err(_) => {
                    let s = String::from_utf8_lossy(&pending).into_owned();
                    on_token(&s);
                    out.push_str(&s);
                    pending.clear();
                }
            }

            batch.clear();
            batch.add(token, n_cur, &[0], true)?;
            n_cur += 1;
            n_gen += 1;
            ctx.decode(&mut batch)?;
        }
        Ok(out)
    }
}

#[derive(Debug)]
pub enum LlmOutcome {
    Text(String),
    ToolCall { id: String, name: String, arguments: String }, // arguments: JSON string, OpenAI style
}

fn tool_names(tools: &[Value]) -> Vec<String> {
    tools.iter()
        .filter_map(|t| t["function"]["name"].as_str().or(t["name"].as_str()))
        .map(String::from)
        .collect()
}

fn parse_tool_call(out: &str) -> Option<(String, Value)> {
    let a = out.find("<tool_call>")? + "<tool_call>".len();
    let b = out[a..].find("</tool_call>").map(|i| a + i).unwrap_or(out.len());
    let v: Value = serde_json::from_str(out[a..b].trim()).ok()?;
    Some((v["name"].as_str()?.to_string(), v["arguments"].clone()))
}

pub async fn local_llm_call<'a>(
    llm: LlmState,
    envs: Arc<HashMap<String, String>>, // unused for now
    debug: bool,
    node_name: &'a str,
    node_details: &'a Node,
    messages: &'a mut Vec<Message>,
) -> Result<LlmOutcome> {
    // clone out; never hold the guard across .await
    let engine = llm.engine.read().unwrap().clone()   
        .ok_or_else(|| anyhow!("[+] local LLM not started"))?;

    // tools: each parsed_tools entry is one OpenAI-format JSON string
    let tools: Vec<Value> = node_details.parsed_tools.iter().flatten()
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect();

    // Message -> (role, content); "tool" results become user messages
    let mut chat: Vec<(String, String)> = messages.iter().map(|m| match m.role.as_str() {
        "tool" => (
            "user".to_string(),
            format!("Tool result (call {}): {}", m.tool_call_id.as_deref().unwrap_or("?"), m.content),
        ),
        r => (r.to_string(), m.content.clone()),
    }).collect();

    if !tools.is_empty() {
        let sys = format!(
            "You can call tools. To call one, reply with ONLY this and nothing else:\n\
             <tool_call>{{\"name\": \"<tool name>\", \"arguments\": {{...}}}}</tool_call>\n\
             If no tool is needed, answer normally.\n\nTools:\n{}",
            serde_json::to_string_pretty(&tools)?
        );
        if chat.first().map_or(false, |(r, _)| r == "system") {
            chat[0].1.push_str("\n\n");
            chat[0].1.push_str(&sys);
        } else {
            chat.insert(0, ("system".into(), sys));
        }
    }

    if llm.busy.swap(true, Ordering::SeqCst) {
        bail!("[+] local LLM is busy");
    }

    llm.cancel.store(false, Ordering::SeqCst);
    let cancel = llm.cancel.clone();

    if debug {
        println!("[{node_name}] local LLM generating...");
    }

    let res = tokio::task::spawn_blocking(move || {
        engine.generate(&chat, 512, &cancel, |t| {
            if debug {
                print!("{t}");
                let _ = std::io::stdout().flush();
            }
        })
    })
    .await;

    llm.busy.store(false, Ordering::SeqCst);   // always reset, success or failure
    let out = res??;                            // JoinError, then generation error
    if debug { println!(); }

    let message = Message {
        role: String::from("assistant"),
        content: out.clone(),
        tool_call_id: None,
        name: None,
        tool_calls: None,
    };

    let _ = save_message(&message.clone(), node_name).await?;
    let _ = save_debug(&serde_json::to_string_pretty(&message)?, node_name).await?;

    println!("\n\n");
    println!("> {}", message.content);
    messages.push(message);

    if let Some((name, args)) = parse_tool_call(&out) {
        if tool_names(&tools).contains(&name) {   // only names from this node's own tool list
            return Ok(LlmOutcome::ToolCall {
                id: format!("call_{}", messages.len()),
                name,
                arguments: args.to_string(),
            });
        }
    }

    Ok(LlmOutcome::Text(out))
}

// #[tokio::main]
// async fn main() -> Result<()> {
//     // Usage: local-llm [model_url] [prompt]
//     let mut args = std::env::args().skip(1);
//     let url = args.next();
//     let prompt = args
//         .next()
//         .unwrap_or_else(|| "Explain what a KV cache is in two sentences.".into());

//     // On mobile, pass the app's private data dir instead.
//     let models_dir = PathBuf::from("models");

//     let path = download_model(&url, &models_dir, |done, total| match total {
//         Some(t) => eprint!("\rdownloading: {:.1}%", done as f64 / t as f64 * 100.0),
//         None => eprint!("\rdownloading: {} MB", done / 1_000_000),
//     })
//     .await?;
//     eprintln!("\nmodel: {}", path.display());

//     // Inference is blocking and CPU/GPU heavy: keep it off the async runtime.
//     tokio::task::spawn_blocking(move || -> Result<()> {
//         let engine = Engine::load(&path)?;
//         engine.generate(&prompt, 256, &AtomicBool::new(false), |piece| {
//             print!("{piece}");
//             let _ = std::io::stdout().flush();
//         })?;
//         println!();
//         Ok(())
//     })
//     .await??;

//     Ok(())
// }

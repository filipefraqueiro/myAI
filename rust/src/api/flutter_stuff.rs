use std::{
    collections::HashMap,
    sync::{
        Mutex,
        MutexGuard,
        OnceLock
    },
};
use tokio::sync::mpsc;
use crate::frb_generated::StreamSink;

// mine
use crate::structs::{
    NodeMessage,
};

// ignore mutex poisoning
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

static NODE_SENDERS: OnceLock<Mutex<HashMap<String, mpsc::Sender<NodeMessage>>>> = OnceLock::new();

pub fn senders() -> &'static Mutex<HashMap<String, mpsc::Sender<NodeMessage>>> {
    NODE_SENDERS.get_or_init(Default::default)
}

async fn send(node: &str, msg: NodeMessage) -> Result<(), String> {
    // clone the sender, drop the lock BEFORE awaiting
    let tx = lock(senders())
        .get(node)
        .cloned()
        .ok_or_else(|| format!("unknown node {node}"))?;

    tx.send(msg).await.map_err(|e| {
        lock(senders()).remove(node); // node loop is dead, prune it
        e.to_string()
    })
}

pub async fn focus_node(node: String) -> Result<(), String> {
    let all: Vec<String> = lock(senders()).keys().cloned().collect();
    for n in all.iter().filter(|n| **n != node) {
        let _ = send(n, NodeMessage::Unfocus).await; // a dead node must not block focus
    }
    send(&node, NodeMessage::Focus).await
}

pub async fn send_to_node(node: String, input: String) -> Result<(), String> {
    send(&node, NodeMessage::Input(input)).await
}

// ---------- events to Dart ----------

pub struct NodeEvent {
    pub node: String,
    pub kind: String,     // "assistant" | "tool" | "error" | "status"
    pub text: String,
}

static EVENTS: OnceLock<Mutex<Option<StreamSink<NodeEvent>>>> = OnceLock::new();

fn events() -> &'static Mutex<Option<StreamSink<NodeEvent>>> {
    EVENTS.get_or_init(|| Mutex::new(None))
}

// use your lock() helper; unwrap() on a poisoned mutex will panic
pub fn subscribe_events(sink: StreamSink<NodeEvent>) {
    *lock(events()) = Some(sink);
}

pub fn emit(node: &str, kind: &str, text: impl Into<String>) {
    let text = text.into();
    log::info!("[{node}] {kind}: {text}");
    let mut g = lock(events());
    if let Some(s) = g.as_ref() {
        if s.add(
            NodeEvent{
                node: node.into(),
                kind: kind.into(),
                text
            }).is_err() {
            *g = None; // Dart listener gone
        }
    }
}



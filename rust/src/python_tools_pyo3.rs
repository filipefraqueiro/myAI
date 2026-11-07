use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};
use serde_json::Value;
use std::ffi::CString;

pub async fn execute_tool(source: &str, func_name: &str, arguments_json: &str) -> Result<String, String> {
    let source = source.to_string();
    let func_name = func_name.to_string();
    let arguments_json = arguments_json.to_string();

    tokio::task::spawn_blocking(move || {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024) // 32 MiB
            .spawn(move || run_python_tool(&source, &func_name, &arguments_json))
            .map_err(|e| format!("failed to spawn python thread: {e}"))?
            .join()
            .map_err(|_| "python execution thread panicked".to_string())?
    })
        .await
        .map_err(|e| format!("spawn_blocking task panicked: {e}"))?
}

pub fn run_python_tool(source: &str, func_name: &str, arguments_json: &str) -> Result<String, String> {
    let code = CString::new(source).map_err(|e| e.to_string())?;

    Python::attach(|py| {
        let module = PyModule::from_code(py, &code, c"tool.py", c"tool")
            .map_err(|e| format!("script error: {e}"))?;

        let func = module
            .getattr(func_name)
            .map_err(|_| format!("function '{func_name}' not found"))?;

        let json = PyModule::import(py, "json").map_err(|e| e.to_string())?;
        let args = json
            .call_method1("loads", (arguments_json,))
            .map_err(|e| format!("invalid args JSON: {e}"))?;

        // {} / {"a":1} -> kwargs, [1,2] -> positional
        let result = if let Ok(kwargs) = args.cast::<PyDict>() {
            func.call((), Some(kwargs))
        } else if let Ok(list) = args.cast::<PyList>() {
            func.call1(list.to_tuple())
        } else {
            func.call1((args,))
        }
        .map_err(|e| format!("runtime error in '{func_name}': {e}"))?;

        json.call_method1("dumps", (result,))
            .and_then(|s| s.extract::<String>())
            .map_err(|e| format!("result not JSON serializable: {e}"))
    })
}

pub async fn execute_tool_from_file(file: &str, func_name: String, arguments_json: String) -> Result<Value, String> {
    let source = tokio::fs::read_to_string(file)
        .await
        .map_err(|e| format!("could not read {file}: {e}"))?;

    let out = tokio::task::spawn_blocking(move || run_python_tool(&source, &func_name, &arguments_json))
        .await
        .map_err(|e| format!("join error: {e}"))??;

    serde_json::from_str(&out).map_err(|e| e.to_string())
}
// use rustpython::Interpreter;
// use rustpython::InterpreterBuilderExt;
// use serde_json::Value;

// #[warn(dead_code)]
// pub async fn execute_tool(source: &str, func_name: &str, arguments_json: &str) -> Result<Value, String> {
//     let source = source.to_string();
//     let func_name = func_name.to_string();
//     let arguments_json = arguments_json.to_string();

//     tokio::task::spawn_blocking(move || {
//         std::thread::Builder::new()
//             .stack_size(32 * 1024 * 1024) // 32 MiB
//             .spawn(move || run_python_tool(&source, &func_name, &arguments_json))
//             .map_err(|e| format!("failed to spawn python thread: {e}"))?
//             .join()
//             .map_err(|_| "python execution thread panicked".to_string())?
//     })
//         .await
//         .map_err(|e| format!("spawn_blocking task panicked: {e}"))?
// }

// fn run_python_tool(source: &str, func_name: &str, arguments_json: &str) -> Result<Value, String> {
//     // println!("Running Tool {} {} {}", source, func_name, arguments_json);
    
//     // let interpreter = Interpreter::without_stdlib(Default::default());

//     let interpreter = Interpreter::builder(Default::default())
//         .init_stdlib()
//         .build();

//     interpreter.enter(|vm| {
//         let scope = vm.new_scope_with_builtins();

//         // Rust -> Python
//         scope
//             .globals
//             .set_item(
//                 "__args_json__",
//                 vm.ctx.new_str(arguments_json).into(),
//                 vm,
//             )
//             .map_err(|e| format!("Failed to set Python arguments: {:?}", e))?;

//         let wrapper = format!(
//             r#"
// {source}

// import json as __json__

// __args__ = __json__.loads(__args_json__)

// __result__ = {func_name}(**__args__)

// __result_json__ = __json__.dumps(__result__)
// "#,
//             source = source,
//             func_name = func_name,
//         );


//         // Execute Python.
//         vm.run_string(
//             scope.clone(),
//             &wrapper,
//             "<tool>".to_owned(),
//         )
//         .map_err(|e| format!("Python execution failed: {:?}", e))?;

//         // Get result.
//         let result_obj = scope
//             .globals
//             .get_item("__result_json__", vm)
//             .map_err(|e| format!("Failed to retrieve Python result: {:?}", e))?;

//         let result_str = result_obj
//             .str(vm)
//             .map_err(|e| format!("Failed to convert Python result: {:?}", e))?
//             .to_string();

//         // Python JSON -> Rust JSON.
//         serde_json::from_str(&result_str)
//             .map_err(|e| format!("Invalid result JSON: {e}"))
//     })
// }

// #[warn(unused)]
// pub async fn execute_tool_from_file(file: &str, func_name: String, arguments_json: String) -> Result<Value, String> {
//     let source = std::fs::read_to_string(file)
//         .map_err(|e| format!("could not read {}: {e}", file))?;

//     let out_tool = run_python_tool(&source, &func_name, &arguments_json);
//     // println!("{:#?}", out_tool);

//     Ok(out_tool?)
// }

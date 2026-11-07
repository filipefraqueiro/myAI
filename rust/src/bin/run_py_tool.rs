// use myai::python_tools::execute_tool;
// use myai::python_tools::execute_tool_from_file;
use myai::python_tools_pyo3::execute_tool_from_file;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tool_out = execute_tool_from_file(&"tools/device.py", "make_web_request".to_string(), r#"{"url": "https://www.mvg.de/api/bgw-pt/v3/lines"}"#.to_string()).await?;
    println!("{:#?}", tool_out);

    Ok(())
}
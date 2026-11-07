use rustpython_parser::{ast, Parse};
use serde_json::{json, Value};

fn python_type_to_json_type(annotation: Option<&ast::Expr>) -> &'static str {
    let Some(expr) = annotation else { return "string" };
    let type_str = expr_to_string(expr);
    if type_str.contains("str") { "string" }
    else if type_str.contains("int") { "integer" }
    else if type_str.contains("float") { "number" }
    else if type_str.contains("bool") { "boolean" }
    else if type_str.contains("List") || type_str.contains("list") { "array" }
    else if type_str.contains("Dict") || type_str.contains("dict") { "object" }
    else { "string" }
}

fn expr_to_string(expr: &ast::Expr) -> String {
    // Debug formatting as a stand-in — fine for now, but treat as fragile.
    // For real robustness, match explicitly on ast::Expr::Name / ast::Expr::Subscript
    // to pull out "Optional[str]" -> inner "str", etc.
    format!("{:?}", expr)
}

fn build_properties(args: &ast::Arguments) -> (serde_json::Map<String, Value>, Vec<String>) {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for arg in &args.args {
        let pname = arg.def.arg.as_str();
        if pname == "self" {
            continue;
        }
        let ptype = python_type_to_json_type(arg.def.annotation.as_deref());
        properties.insert(pname.to_string(), json!({ "type": ptype }));

        // ArgWithDefault carries its own default directly — no separate
        // parallel `defaults` list to align by index.
        if arg.default.is_none() {
            required.push(pname.to_string());
        }
    }

    (properties, required)
}

fn extract_docstring(body: &[ast::Stmt]) -> Option<String> {
    if let Some(ast::Stmt::Expr(e)) = body.first() {
        if let ast::Expr::Constant(c) = e.value.as_ref() {
            if let ast::Constant::Str(s) = &c.value {
                return Some(s.clone());
            }
        }
    }
    None
}

/// Returns Result instead of Option so failures are visible instead of
/// silently collapsing to "nothing happened".
pub fn extract_function_schema(source: &str, func_name: &str) -> Result<Value, String> {
    let module = ast::Suite::parse(source, "<tool>")
        .map_err(|e| format!("parse error: {e}"))?;

    let mut found_names: Vec<String> = Vec::new();

    for stmt in module {
        match stmt {
            ast::Stmt::FunctionDef(f) => {
                found_names.push(f.name.to_string());
                if f.name.as_str() != func_name {
                    continue;
                }
                let docstring = extract_docstring(&f.body);
                let (properties, required) = build_properties(&f.args);
                return Ok(json!({
                    "description": docstring.unwrap_or_else(|| format!("{} tool", func_name)),
                    "input_schema": {
                        "type": "object",
                        "properties": properties,
                        "required": required
                    }
                }));
            }
            ast::Stmt::AsyncFunctionDef(f) => {
                found_names.push(format!("async {}", f.name));
                if f.name.as_str() != func_name {
                    continue;
                }
                let docstring = extract_docstring(&f.body);
                let (properties, required) = build_properties(&f.args);
                return Ok(json!({
                    "description": docstring.unwrap_or_else(|| format!("{} tool", func_name)),
                    "input_schema": {
                        "type": "object",
                        "properties": properties,
                        "required": required
                    }
                }));
            }
            _ => {}
        }
    }

    Err(format!(
        "function '{}' not found at top level. functions found in file: {:?}",
        func_name, found_names
    ))
}

/// Gemini is stricter than Anthropic/OpenAI about schemas:
/// - `parameters` must be omitted entirely when there are no properties
/// - `required` must be omitted when empty
/// - `array` types must declare `items`
fn gemini_function_declaration(name: &str, description: &Value, params: &Value) -> Value {
    let mut decl = json!({
        "name": name,
        "description": description
    });

    let Some(props) = params["properties"].as_object().filter(|p| !p.is_empty()) else {
        return decl; // zero-arg function: no `parameters` key
    };

    let mut props = props.clone();
    for (_, prop) in props.iter_mut() {
        if prop["type"] == "array" && prop.get("items").is_none() {
            prop["items"] = json!({ "type": "string" });
        }
    }

    let mut parameters = json!({
        "type": "object",
        "properties": props
    });

    if let Some(req) = params["required"].as_array().filter(|r| !r.is_empty()) {
        parameters["required"] = json!(req);
    }

    decl["parameters"] = parameters;
    decl
}

/// Gemini wants all declarations grouped under a single tool entry.
/// Pass the result as the `"tools"` field of the request body.
// pub fn gemini_tools_payload(declarations: Vec<Value>) -> Value {
//     json!([{ "functionDeclarations": declarations }])
// }

pub fn wrap_for_provider(name: &str, schema: &Value, provider: &str) -> Value {
    let description = schema["description"].clone();
    let params = schema["input_schema"].clone();

    match provider {
        "anthropic" => json!({
            "name": name,
            "description": description,
            "input_schema": params
        }),
        "openai" | "openai_compat" => json!({
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": params
            }
        }),
        // Returns a single function declaration; group with gemini_tools_payload().
        "gemini" | "google" => gemini_function_declaration(name, &description, &params),
        _ => json!({
            "name": name,
            "description": description,
            "input_schema": params
        }),
    }
}
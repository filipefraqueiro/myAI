use std::collections::HashMap;
use std::sync::Arc;

pub fn check_for_secrets(mut input: String, envs: Arc<HashMap<String, String>>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // println!("{:#?}", envs);
    // println!("{:#?}", input);

    for (key, value) in envs.iter() {
        if !value.is_empty() {
            input = input.replace(value, key);
        }
    }

    // println!("{:#?}", input);

    Ok(input)
}
//! Fail-closed, text-only read-tool contract for private local Dev completions.
//! This does not authorize filesystem access: the host transcript/read guard does that.
use crate::app::{AppError, AppResult};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

fn invalid() -> AppError {
    AppError::new(
        "PROVIDER_PROTOCOL",
        "Unsupported local inspection completion contract",
    )
}
fn object<'a>(v: &'a Value, allowed: &[&str]) -> AppResult<&'a Map<String, Value>> {
    v.as_object()
        .filter(|o| o.keys().all(|k| allowed.contains(&k.as_str())))
        .ok_or_else(invalid)
}
fn text(v: &Value) -> AppResult<String> {
    if let Some(s) = v.as_str() {
        return Ok(s.to_owned());
    }
    let parts = v.as_array().filter(|a| !a.is_empty()).ok_or_else(invalid)?;
    let mut result = String::new();
    for part in parts {
        object(part, &["type", "text"])?;
        if part["type"] != "text" {
            return Err(invalid());
        }
        result.push_str(part["text"].as_str().ok_or_else(invalid)?);
    }
    Ok(result)
}
fn id(v: &Value) -> AppResult<&str> {
    v.as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
        .ok_or_else(invalid)
}

/// Validate exact reader arguments; defaults are enforced by the native reader.
/// Paths are lexical here; descriptor-relative containment is independently required at I/O.
pub fn read_arguments(value: &Value) -> AppResult<Value> {
    let args = object(value, &["filePath", "offset", "limit"])?;
    let path = args
        .get("filePath")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let components: Vec<_> = path.split('/').collect();
    if path.len() > 4096
        || components.len() < 4
        || components.len() > 36
        || components[..3] != ["", "workspace", "repos"]
        || !(0..16).any(|i| components[3] == format!("r{i}"))
        || components[4..].iter().any(|c| {
            c.is_empty()
                || *c == "."
                || *c == ".."
                || c.eq_ignore_ascii_case(".git")
                || c.chars().any(char::is_control)
                || c.contains('\\')
        })
    {
        return Err(invalid());
    }
    for (key, max) in [("offset", 20_000), ("limit", 32)] {
        if args
            .get(key)
            .is_some_and(|v| v.as_u64().is_none_or(|n| !(1..=max).contains(&n)))
        {
            return Err(invalid());
        }
    }
    Ok(value.clone())
}
fn calls(value: &Value, seen: &mut HashSet<String>) -> AppResult<Value> {
    let calls = value
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 4)
        .ok_or_else(invalid)?;
    let mut result = Vec::new();
    for call in calls {
        object(call, &["id", "type", "function"])?;
        let call_id = id(&call["id"])?;
        if call["type"] != "function" || !seen.insert(call_id.to_owned()) || seen.len() > 16 {
            return Err(invalid());
        }
        object(&call["function"], &["name", "arguments"])?;
        if call["function"]["name"] != "read" {
            return Err(invalid());
        }
        let raw = call["function"]["arguments"]
            .as_str()
            .filter(|s| s.len() <= 8192)
            .ok_or_else(invalid)?;
        // Duplicate JSON keys must not give the guard and native reader different arguments.
        let args = crate::cloud::unique_json(raw.as_bytes()).map_err(|_| invalid())?;
        read_arguments(&args)?;
        result.push(call.clone());
    }
    Ok(Value::Array(result))
}
fn schema(tools: &Value) -> AppResult<()> {
    if serde_json::to_vec(tools).map_err(|_| invalid())?.len() > 8192 {
        return Err(invalid());
    }
    let tools = tools
        .as_array()
        .filter(|a| a.len() == 1)
        .ok_or_else(invalid)?;
    let tool = &tools[0];
    object(tool, &["type", "function"])?;
    if tool["type"] != "function" {
        return Err(invalid());
    }
    let function = &tool["function"];
    object(function, &["name", "description", "parameters", "strict"])?;
    if function["name"] != "read"
        || function
            .get("description")
            .is_some_and(|v| v.as_str().is_none())
        || function.get("strict").is_some_and(|v| !v.is_boolean())
    {
        return Err(invalid());
    }
    let parameters = &function["parameters"];
    object(
        parameters,
        &[
            "type",
            "properties",
            "required",
            "additionalProperties",
            "$schema",
        ],
    )?;
    if parameters["type"] != "object"
        || parameters
            .get("additionalProperties")
            .is_some_and(|v| v != false)
        || parameters
            .get("$schema")
            .is_some_and(|v| v.as_str() != Some("http://json-schema.org/draft-07/schema#"))
    {
        return Err(invalid());
    }
    let props = object(&parameters["properties"], &["filePath", "offset", "limit"])?;
    if !props.contains_key("filePath") {
        return Err(invalid());
    }
    let required = parameters["required"].as_array().ok_or_else(invalid)?;
    let mut keys = HashSet::new();
    for key in required {
        let key = key.as_str().ok_or_else(invalid)?;
        if !props.contains_key(key) || !keys.insert(key) {
            return Err(invalid());
        }
    }
    if !keys.contains("filePath") {
        return Err(invalid());
    }
    for (name, prop) in props {
        object(prop, &["type", "description", "minimum", "maximum"])?;
        let valid_type = if name == "filePath" {
            prop["type"] == "string"
        } else {
            prop["type"] == "integer" || prop["type"] == "number"
        };
        if !valid_type
            || prop
                .get("description")
                .is_some_and(|v| v.as_str().is_none())
        {
            return Err(invalid());
        }
        if name == "filePath" {
            if prop.get("minimum").is_some() || prop.get("maximum").is_some() {
                return Err(invalid());
            }
        } else {
            let max = if name == "limit" { 32 } else { 20_000 };
            let min = prop
                .get("minimum")
                .map(Value::as_u64)
                .unwrap_or(Some(1))
                .ok_or_else(invalid)?;
            let upper = prop
                .get("maximum")
                .map(Value::as_u64)
                .unwrap_or(Some(max))
                .ok_or_else(invalid)?;
            if min < 1 || min > upper || upper > max {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Normalize only text arrays; preserve validated call argument strings verbatim.
/// Never trim history/task.
/// The service separately binds this transcript to the admitted job and issued reads.
pub fn validate_request(value: &Value) -> AppResult<Value> {
    if serde_json::to_vec(value).map_err(|_| invalid())?.len() > 128 * 1024 {
        return Err(invalid());
    }
    object(
        value,
        &[
            "model",
            "stream",
            "messages",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "temperature",
            "top_p",
            "max_tokens",
            "max_completion_tokens",
            "stop",
            "seed",
            "presence_penalty",
            "frequency_penalty",
            "stream_options",
            "n",
        ],
    )?;
    if value["model"] != "selected" || value["stream"] != true {
        return Err(invalid());
    }
    schema(&value["tools"])?;
    if let Some(options) = value.get("stream_options") {
        object(options, &["include_usage"])?;
        if options
            .get("include_usage")
            .is_some_and(|v| !v.is_boolean())
        {
            return Err(invalid());
        }
    }
    for (key, min, max) in [
        ("temperature", 0.0, 2.0),
        ("top_p", 0.0, 1.0),
        ("presence_penalty", -2.0, 2.0),
        ("frequency_penalty", -2.0, 2.0),
    ] {
        if value.get(key).is_some_and(|v| {
            v.as_f64()
                .is_none_or(|n| !n.is_finite() || n < min || n > max)
        }) {
            return Err(invalid());
        }
    }
    for key in ["max_tokens", "max_completion_tokens"] {
        if value
            .get(key)
            .is_some_and(|v| v.as_u64().is_none_or(|n| n == 0 || n > 1024))
        {
            return Err(invalid());
        }
    }
    if value.get("n").is_some_and(|v| v.as_u64() != Some(1))
        || value.get("seed").is_some_and(|v| v.as_i64().is_none())
        || value
            .get("parallel_tool_calls")
            .is_some_and(|v| !v.is_boolean())
    {
        return Err(invalid());
    }
    if let Some(stop) = value.get("stop") {
        let valid = |s: &Value| s.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256);
        if !stop.is_null()
            && !valid(stop)
            && !stop
                .as_array()
                .is_some_and(|a| a.len() <= 4 && a.iter().all(valid))
        {
            return Err(invalid());
        }
    }
    if let Some(choice) = value.get("tool_choice") {
        if choice != "auto" && choice != "required" && choice != "none" {
            object(choice, &["type", "function"])?;
            object(&choice["function"], &["name"])?;
            if choice["type"] != "function" || choice["function"]["name"] != "read" {
                return Err(invalid());
            }
        }
    }
    let input = value["messages"]
        .as_array()
        .filter(|a| (2..=42).contains(&a.len()))
        .ok_or_else(invalid)?;
    let mut messages = Vec::new();
    let mut seen = HashSet::new();
    let mut pending = HashSet::new();
    for (index, message) in input.iter().enumerate() {
        object(message, &["role", "content", "tool_calls", "tool_call_id"])?;
        let role = message["role"].as_str().ok_or_else(invalid)?;
        let mut normalized = json!({"role":role});
        match role {
            "system" if index == 0 => {
                let content = text(&message["content"])?;
                if content.len() > 2048
                    || content != crate::dev_local::SYSTEM_PROMPT
                    || message.get("tool_calls").is_some()
                    || message.get("tool_call_id").is_some()
                {
                    return Err(invalid());
                }
                normalized["content"] = content.into();
            }
            "user" if index == 1 => {
                let content = text(&message["content"])?;
                if content.trim().is_empty()
                    || message.get("tool_calls").is_some()
                    || message.get("tool_call_id").is_some()
                {
                    return Err(invalid());
                }
                normalized["content"] = content.into();
            }
            "assistant"
                if index > 1 && pending.is_empty() && input[index - 1]["role"] != "assistant" =>
            {
                if message.get("tool_call_id").is_some() {
                    return Err(invalid());
                }
                let validated = calls(&message["tool_calls"], &mut seen)?;
                for call in validated.as_array().ok_or_else(invalid)? {
                    pending.insert(id(&call["id"])?.to_owned());
                }
                normalized["tool_calls"] = validated;
                normalized["content"] = if message["content"].is_null() {
                    Value::Null
                } else {
                    text(&message["content"])?.into()
                };
            }
            "tool" if !pending.is_empty() => {
                if message.get("tool_calls").is_some()
                    || !pending.remove(id(&message["tool_call_id"])?)
                {
                    return Err(invalid());
                }
                let content = text(&message["content"])?;
                if content.len() > 2048 {
                    return Err(invalid());
                }
                normalized["content"] = content.into();
                normalized["tool_call_id"] = message["tool_call_id"].clone();
            }
            _ => return Err(invalid()),
        }
        messages.push(normalized);
    }
    if input[0]["role"] != "system"
        || input[1]["role"] != "user"
        || !pending.is_empty()
        || !matches!(
            input.last().and_then(|m| m["role"].as_str()),
            Some("user" | "tool")
        )
    {
        return Err(invalid());
    }
    let mut result = value.clone();
    result["messages"] = messages.into();
    Ok(result)
}

/// Validate native identity and strip all reasoning, usage and native metadata.
pub fn validate_response(alias: &str, value: &Value) -> AppResult<Value> {
    object(
        value,
        &[
            "id",
            "object",
            "created",
            "model",
            "choices",
            "usage",
            "system_fingerprint",
            "timings",
            "error",
        ],
    )?;
    if value["model"] != alias || value.get("error").is_some_and(|v| !v.is_null()) {
        return Err(invalid());
    }
    let choices = value["choices"]
        .as_array()
        .filter(|a| a.len() == 1)
        .ok_or_else(invalid)?;
    let choice = &choices[0];
    object(choice, &["index", "message", "finish_reason", "logprobs"])?;
    if choice["index"] != 0 {
        return Err(invalid());
    }
    let message = &choice["message"];
    object(
        message,
        &[
            "role",
            "content",
            "tool_calls",
            "reasoning_content",
            "reasoning",
            "function_call",
            "refusal",
        ],
    )?;
    if message["role"] != "assistant"
        || message.get("function_call").is_some_and(|v| !v.is_null())
        || message.get("refusal").is_some_and(|v| !v.is_null())
    {
        return Err(invalid());
    }
    let finish = choice["finish_reason"].as_str().ok_or_else(invalid)?;
    let mut result = json!({"role":"assistant"});
    match finish {
        "stop" => {
            if message
                .get("tool_calls")
                .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
            {
                return Err(invalid());
            }
            let content = message["content"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && s.len() <= 64 * 1024)
                .ok_or_else(invalid)?;
            result["content"] = content.into();
        }
        "tool_calls" => {
            result["tool_calls"] = calls(&message["tool_calls"], &mut HashSet::new())?;
            result["content"] = if message["content"].is_null() {
                Value::Null
            } else {
                message["content"]
                    .as_str()
                    .filter(|s| s.len() <= 64 * 1024)
                    .ok_or_else(invalid)?
                    .into()
            };
        }
        "length" => {
            return Err(AppError::new(
                "MODEL_TRUNCATED",
                "Local inspection reached its generation cap; no replay",
            ))
        }
        _ => return Err(invalid()),
    }
    Ok(json!({"model":"selected","choices":[{"index":0,"finish_reason":finish,"message":result}]}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"model":"selected","stream":true,"messages":[{"role":"system","content":crate::dev_local::SYSTEM_PROMPT},{"role":"user","content":[{"type":"text","text":"Inspect files"}]}],"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object","properties":{"filePath":{"type":"string"},"offset":{"type":"integer","minimum":1,"maximum":20000},"limit":{"type":"integer","minimum":1,"maximum":32}},"required":["filePath"],"additionalProperties":false}}}]})
    }
    fn call() -> Value {
        json!({"id":"call_1","type":"function","function":{"name":"read","arguments":"{\"filePath\":\"/workspace/repos/r0/README.md\",\"limit\":32}"}})
    }
    #[test]
    fn text_normalization_and_exact_trusted_system() {
        let mut r = request();
        assert_eq!(
            validate_request(&r).unwrap()["messages"][1]["content"],
            "Inspect files"
        );
        r["messages"][0]["content"] = "untrusted".into();
        assert!(validate_request(&r).is_err());
    }
    #[test]
    fn exact_checkout_roots_are_allowed_without_relaxing_containment() {
        for index in 0..16 {
            let args = json!({"filePath":format!("/workspace/repos/r{index}")});
            assert_eq!(read_arguments(&args).unwrap(), args);
        }
        for path in [
            "/workspace/repos",
            "/workspace/repos/r16",
            "/workspace/repos/r00",
            "/workspace/repos/r-1",
            "/workspace/repos/r0-extra",
            "/outside/repos/r0",
            "/workspace/repos/r0/",
            "/workspace/repos/r0/.",
            "/workspace/repos/r0/..",
            "/workspace/repos/r0/.git",
        ] {
            assert!(read_arguments(&json!({"filePath":path})).is_err(), "{path}");
        }
    }
    #[test]
    fn read_bounds_and_unknown_fields_fail_closed() {
        for path in [
            "/workspace/repos/r16/a",
            "/workspace/repos/r0/../a",
            "/workspace/repos/r0/.git/config",
            "/workspace/repos/r0//a",
            "/workspace/repos/r00/a",
            "relative",
            "/workspace/repos/r0/./a",
        ] {
            assert!(read_arguments(&json!({"filePath":path})).is_err());
        }
        for args in [
            json!({"filePath":"/workspace/repos/r15/a","offset":0}),
            json!({"filePath":"/workspace/repos/r0/a","limit":33}),
            json!({"filePath":"/workspace/repos/r0/a","command":"cat"}),
        ] {
            assert!(read_arguments(&args).is_err());
        }
        assert!(read_arguments(
            &json!({"filePath":"/workspace/repos/r15/a","offset":20000,"limit":32})
        )
        .is_ok());
    }
    #[test]
    fn tool_transcript_requires_complete_unique_results() {
        let mut r = request();
        r["messages"].as_array_mut().unwrap().extend([
            json!({"role":"assistant","content":null,"tool_calls":[call()]}),
            json!({"role":"tool","tool_call_id":"call_1","content":"evidence"}),
        ]);
        assert!(validate_request(&r).is_ok());
        r["messages"][3]["tool_call_id"] = "foreign".into();
        assert!(validate_request(&r).is_err());
        r["messages"].as_array_mut().unwrap().pop();
        assert!(validate_request(&r).is_err());
    }
    #[test]
    fn schema_request_and_size_limits() {
        let mut r = request();
        r["tools"][0]["function"]["name"] = "shell".into();
        assert!(validate_request(&r).is_err());
        let mut r = request();
        r["endpoint"] = "http://example.invalid".into();
        assert!(validate_request(&r).is_err());
        let mut r = request();
        r["messages"][1]["content"] = "x".repeat(128 * 1024).into();
        assert!(validate_request(&r).is_err());
        let mut r = request();
        r["tools"][0]["function"]["description"] = "x".repeat(8192).into();
        assert!(validate_request(&r).is_err());
    }
    #[test]
    fn optional_compatible_fields_and_text_only_parts() {
        let mut r = request();
        r["stream_options"] = json!({"include_usage":true});
        r["max_tokens"] = 1024.into();
        r["temperature"] = 0.5.into();
        r["tool_choice"] = "auto".into();
        r["parallel_tool_calls"] = false.into();
        assert!(validate_request(&r).is_ok());
        r["messages"][1]["content"] =
            json!([{"type":"image_url","image_url":{"url":"file:///secret"}}]);
        assert!(validate_request(&r).is_err());
        let mut r = request();
        r["tools"][0]["function"]["parameters"]["properties"]["command"] = json!({"type":"string"});
        assert!(validate_request(&r).is_err());
    }
    #[test]
    fn total_call_budget_and_argument_identity() {
        let mut r = request();
        for index in 0..16 {
            let mut c = call();
            c["id"] = format!("call_{index}").into();
            r["messages"].as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":null,"tool_calls":[c]}),
                json!({"role":"tool","tool_call_id":format!("call_{index}"),"content":"evidence"}),
            ]);
        }
        assert!(validate_request(&r).is_ok());
        let mut c = call();
        c["id"] = "call_16".into();
        r["messages"].as_array_mut().unwrap().extend([
            json!({"role":"assistant","content":null,"tool_calls":[c]}),
            json!({"role":"tool","tool_call_id":"call_16","content":"evidence"}),
        ]);
        assert!(validate_request(&r).is_err());
        let mut c = call();
        c["function"]["arguments"] = "{  \"filePath\": \"/workspace/repos/r0/a\" }".into();
        assert_eq!(
            calls(&json!([c.clone()]), &mut HashSet::new()).unwrap()[0],
            c
        );
        c["function"]["arguments"] =
            "{\"filePath\":\"/workspace/repos/r0/a\",\"filePath\":\"/workspace/repos/r0/b\"}"
                .into();
        assert!(calls(&json!([c]), &mut HashSet::new()).is_err());
    }
    #[test]
    fn per_round_calls_and_unknown_response_fields() {
        let mut r = json!({"model":"digest","choices":[{"index":0,"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[]}}]});
        for i in 0..4 {
            let mut c = call();
            c["id"] = format!("c_{i}").into();
            r["choices"][0]["message"]["tool_calls"]
                .as_array_mut()
                .unwrap()
                .push(c);
        }
        assert!(validate_response("digest", &r).is_ok());
        let mut c = call();
        c["id"] = "fifth".into();
        r["choices"][0]["message"]["tool_calls"]
            .as_array_mut()
            .unwrap()
            .push(c);
        assert!(validate_response("digest", &r).is_err());
        r["choices"][0]["message"]["tool_calls"]
            .as_array_mut()
            .unwrap()
            .pop();
        r["unexpected"] = true.into();
        assert!(validate_response("digest", &r).is_err());
    }
    #[test]
    fn response_strips_reasoning_and_rejects_incomplete_or_foreign() {
        let mut r = json!({"model":"digest","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"answer","reasoning_content":"private"}}]});
        let clean = validate_response("digest", &r).unwrap();
        assert_eq!(clean["model"], "selected");
        assert!(clean["choices"][0]["message"]
            .get("reasoning_content")
            .is_none());
        assert!(validate_response("other", &r).is_err());
        r["choices"][0]["message"]["content"] = Value::Null;
        assert!(validate_response("digest", &r).is_err());
        r["choices"][0]["finish_reason"] = "tool_calls".into();
        r["choices"][0]["message"]["tool_calls"] = json!([call()]);
        assert!(validate_response("digest", &r).is_ok());
        r["choices"][0]["message"]["tool_calls"] = json!([call(), call()]);
        assert!(validate_response("digest", &r).is_err());
        r["choices"][0]["finish_reason"] = "length".into();
        assert_eq!(
            validate_response("digest", &r).unwrap_err().code,
            "MODEL_TRUNCATED"
        );
    }
}

use super::*;

fn caps(third: Option<Value>) -> Value {
    let mut result =
        json!({"chat_template_caps":{"supports_tools":true,"supports_tool_calls":true}});
    if let Some(value) = third {
        result["chat_template_caps"]["supports_tool_responses"] = value;
    }
    result
}

// Deliberately omit call IDs: the pinned native template need not render them.
fn render(request: &Value, string_arguments: bool) -> Value {
    let mut prompt = format!(
        "{}\n<tools>\n{}\n</tools>\n",
        crate::dev_local::SYSTEM_PROMPT,
        request["tools"][0]["function"]
    );
    for message in request["messages"].as_array().unwrap().iter().skip(1) {
        if let Some(text) = message["content"].as_str() {
            if message["role"] == "tool" {
                prompt.push_str("<tool_response>\n");
            }
            prompt.push_str(text);
            prompt.push('\n');
            if message["role"] == "tool" {
                prompt.push_str("</tool_response>\n");
            }
        }
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                let arguments = if string_arguments {
                    call["function"]["arguments"].clone()
                } else {
                    crate::cloud::unique_json(
                        call["function"]["arguments"].as_str().unwrap().as_bytes(),
                    )
                    .unwrap()
                };
                prompt.push_str("<tool_call>\n");
                prompt.push_str(&json!({"name":"read","arguments":arguments}).to_string());
                prompt.push_str("\n</tool_call>\n");
            }
        }
    }
    json!({"prompt":prompt})
}

fn text_evidence(probe: &InspectionProbe) -> Vec<String> {
    probe
        .evidence
        .iter()
        .filter_map(|item| match item {
            ProbeEvidence::Text(text) | ProbeEvidence::ToolResult(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn required_props_are_objects_and_exact_booleans() {
    for props in [
        Value::Null,
        json!([]),
        json!({}),
        json!({"chat_template_caps":[]}),
    ] {
        assert!(missing_response_capability(&props, RuntimeProfile::B11429).is_err());
    }
    for key in ["supports_tools", "supports_tool_calls"] {
        for bad in [
            Value::Null,
            json!(false),
            json!(1),
            json!("true"),
            json!({}),
        ] {
            let mut props = caps(None);
            props["chat_template_caps"][key] = bad;
            assert!(missing_response_capability(&props, RuntimeProfile::B11429).is_err());
        }
        let mut props = caps(None);
        props["chat_template_caps"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(missing_response_capability(&props, RuntimeProfile::B11429).is_err());
    }
}

#[test]
fn absent_third_requires_packaged_profile_not_native_version_text() {
    let mut props = caps(None);
    props["build_info"] = json!("b11429 d81235049");
    assert!(missing_response_capability(&props, RuntimeProfile::Unknown).is_err());
    assert!(missing_response_capability(&props, RuntimeProfile::B11429).unwrap());
    assert!(
        !missing_response_capability(&caps(Some(json!(true))), RuntimeProfile::Unknown).unwrap()
    );
}

#[test]
fn explicit_false_null_and_nonboolean_third_never_take_exception() {
    for bad in [
        Value::Null,
        json!(false),
        json!(1),
        json!("true"),
        json!([]),
        json!({}),
    ] {
        assert!(missing_response_capability(&caps(Some(bad)), RuntimeProfile::B11429).is_err());
    }
}

#[test]
fn status_malformed_duplicate_and_nonobject_capabilities_are_rejected() {
    for status in [201, 204, 400, 401, 404, 500, 503] {
        assert_eq!(
            parse_inspection_json(status, b"{}").unwrap_err().code,
            "UNSUPPORTED"
        );
    }
    for bytes in [
        b"not-json".as_slice(),
        b"null",
        b"[]",
        b"{}{}",
        br#"{"chat_template_caps":{},"chat_template_caps":{}}"#,
        br#"{"chat_template_caps":{"supports_tools":false,"supports_tools":true}}"#,
    ] {
        assert_eq!(
            parse_inspection_json(200, bytes).unwrap_err().code,
            "PROVIDER_PROTOCOL"
        );
    }
}

#[test]
fn probe_is_broker_valid_and_covers_call_counts_content_and_argument_shapes() {
    let probe = InspectionProbe::new().unwrap();
    let mut request = probe.request.clone();
    request
        .as_object_mut()
        .unwrap()
        .remove("add_generation_prompt");
    request["model"] = json!("selected");
    request["stream"] = json!(true);
    let normalized = crate::dev_completion::validate_request(&request).unwrap();
    let messages = normalized["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 25);
    let assistants: Vec<_> = messages
        .iter()
        .filter(|m| m["role"] == "assistant")
        .collect();
    assert!(assistants[0]["content"].is_null());
    assert_eq!(assistants[1]["content"], "");
    assert!(assistants[2]["content"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    assert_eq!(assistants.len(), crate::dev_local::MAX_ROUNDS as usize - 1);
    for (assistant, count) in assistants.iter().zip([1, 2, 3, 4, 1, 1, 4]) {
        assert_eq!(assistant["tool_calls"].as_array().unwrap().len(), count);
    }
    assert_eq!(
        assistants
            .iter()
            .map(|m| m["tool_calls"].as_array().unwrap().len())
            .sum::<usize>(),
        crate::dev_local::MAX_READS as usize
    );
    let shapes: BTreeSet<_> = assistants[3]["tool_calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| {
            let value = crate::cloud::unique_json(
                call["function"]["arguments"].as_str().unwrap().as_bytes(),
            )
            .unwrap();
            (value.get("offset").is_some(), value.get("limit").is_some())
        })
        .collect();
    assert_eq!(shapes.len(), 4);
    // Text-array forms normalize to the same template input, not a new authority.
    for message in request["messages"].as_array_mut().unwrap() {
        if let Some(text) = message["content"].as_str() {
            message["content"] = json!([{"type":"text","text":text}]);
        }
    }
    assert_eq!(
        crate::dev_completion::validate_request(&request).unwrap()["messages"],
        normalized["messages"]
    );
}

#[test]
fn controlled_history_passes_actual_broker_without_any_io_or_generation() {
    let probe = InspectionProbe::new().unwrap();
    let messages = probe.request["messages"].as_array().unwrap();
    let mut broker =
        crate::dev_broker::Broker::new(messages[1]["content"].as_str().unwrap().into(), 1).unwrap();
    let request = |prefix: &[Value]| {
        json!({"model":"selected","stream":true,"messages":prefix,
        "tools":probe.request["tools"],"tool_choice":"auto","parallel_tool_calls":false})
    };
    broker.request(&request(&messages[..2])).unwrap();
    let mut index = 2;
    while index < messages.len() {
        let assistant = &messages[index];
        broker
            .response(&json!({"model":"selected","choices":[{"index":0,
            "finish_reason":"tool_calls","message":assistant}]}))
            .unwrap();
        index += 1;
        while index < messages.len() && messages[index]["role"] == "tool" {
            let tool = &messages[index];
            let id = tool["tool_call_id"].as_str().unwrap();
            let call = assistant["tool_calls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|call| call["id"] == id)
                .unwrap();
            let args = crate::cloud::unique_json(
                call["function"]["arguments"].as_str().unwrap().as_bytes(),
            )
            .unwrap();
            broker.authorize_read(id, &args).unwrap();
            broker
                .record_read(id, tool["content"].as_str().unwrap().into())
                .unwrap();
            index += 1;
        }
        broker.request(&request(&messages[..index])).unwrap();
    }
}

#[test]
fn semantic_evidence_supports_every_four_call_result_permutation() {
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if order.iter().collect::<BTreeSet<_>>().len() != 4 {
                        continue;
                    }
                    let mut probe = InspectionProbe::new().unwrap();
                    let messages = probe.request["messages"].as_array_mut().unwrap();
                    let start = messages.len() - 4;
                    let original = messages[start..].to_vec();
                    let evidence_start = probe.evidence.len() - 4;
                    for (index, source) in order.into_iter().enumerate() {
                        messages[start + index] = original[source].clone();
                        probe.evidence[evidence_start + index] = ProbeEvidence::ToolResult(
                            original[source]["content"].as_str().unwrap().into(),
                        );
                    }
                    probe.verify(&render(&probe.request, false)).unwrap();
                }
            }
        }
    }
}

#[test]
fn semantic_probe_accepts_object_or_string_arguments_without_call_ids() {
    let probe = InspectionProbe::new().unwrap();
    for strings in [false, true] {
        let rendered = render(&probe.request, strings);
        assert!(!rendered["prompt"].as_str().unwrap().contains("schema_"));
        probe.verify(&rendered).unwrap();
    }
}

#[test]
fn dropped_altered_duplicate_or_reordered_text_evidence_is_rejected() {
    let probe = InspectionProbe::new().unwrap();
    let rendered = render(&probe.request, false);
    let prompt = rendered["prompt"].as_str().unwrap();
    let texts = text_evidence(&probe);
    for text in &texts {
        for replacement in [String::new(), format!("{text}{text}"), "foreign".into()] {
            assert!(probe
                .verify(&json!({"prompt":prompt.replace(text, &replacement)}))
                .is_err());
        }
    }
    for pair in texts.windows(2) {
        let swapped = prompt
            .replace(&pair[0], "TEMPORARY_SCHEMA_MARKER")
            .replace(&pair[1], &pair[0])
            .replace("TEMPORARY_SCHEMA_MARKER", &pair[1]);
        assert!(probe.verify(&json!({"prompt":swapped})).is_err());
    }
}

#[test]
fn definition_arguments_system_and_call_structure_must_survive() {
    let probe = InspectionProbe::new().unwrap();
    let rendered = render(&probe.request, false);
    let prompt = rendered["prompt"].as_str().unwrap();
    let definition = probe.request["tools"][0]["function"].to_string();
    for replacement in [
        "{}".into(),
        definition.replace("20000", "20001"),
        format!("{definition}{definition}"),
    ] {
        assert!(probe
            .verify(&json!({"prompt":prompt.replace(&definition, &replacement)}))
            .is_err());
    }
    assert!(probe
        .verify(&json!({"prompt":prompt.replace(crate::dev_local::SYSTEM_PROMPT, "")}))
        .is_err());
    for item in &probe.evidence {
        if let ProbeEvidence::Call(call) = item {
            let raw = call.to_string();
            let duplicate = format!(
                "{{\"name\":\"read\",\"name\":\"read\",\"arguments\":{}}}",
                call["arguments"]
            );
            for replacement in [
                raw.replace("SMITH_ARGS", "FOREIGN_ARGS"),
                duplicate,
                "{}".into(),
            ] {
                assert!(probe
                    .verify(&json!({"prompt":prompt.replace(&raw, &replacement)}))
                    .is_err());
            }
        }
    }
}

#[test]
fn losing_native_tool_structure_is_not_proof_even_when_all_content_survives() {
    let probe = InspectionProbe::new().unwrap();
    let rendered = render(&probe.request, false);
    let prompt = rendered["prompt"].as_str().unwrap();
    for tag in [
        "<tools>",
        "</tools>",
        "<tool_call>",
        "</tool_call>",
        "<tool_response>",
        "</tool_response>",
    ] {
        assert!(
            probe
                .verify(&json!({"prompt":prompt.replace(tag, "")}))
                .is_err(),
            "{tag}"
        );
    }
    let plaintext = prompt
        .replace("<tool_response>", "")
        .replace("</tool_response>", "");
    assert!(probe.verify(&json!({"prompt":plaintext})).is_err());
}

#[test]
fn json_echo_missing_prompt_malformed_or_oversized_rendering_is_not_semantic_proof() {
    let probe = InspectionProbe::new().unwrap();
    for value in [
        json!({}),
        json!({"prompt":null}),
        json!({"prompt":42}),
        json!({"prompt":probe.request.to_string()}),
        json!({"prompt":"x".repeat(65537)}),
    ] {
        assert!(probe.verify(&value).is_err());
    }
    let mut prompt = render(&probe.request, false)["prompt"]
        .as_str()
        .unwrap()
        .to_owned();
    prompt.push_str(&"{}".repeat(129));
    assert!(probe.verify(&json!({"prompt":prompt})).is_err());
}

#[test]
fn success_cache_is_per_instance_and_still_rechecks_props() {
    let mut cached = false;
    let mut calls = Vec::new();
    for _ in 0..2 {
        inspection_support(RuntimeProfile::B11429, &mut cached, |method, path, body| {
            calls.push((method.to_owned(), path.to_owned()));
            Ok(if path == "/props" {
                caps(None)
            } else {
                render(body.unwrap(), false)
            })
        })
        .unwrap();
    }
    assert!(cached);
    assert_eq!(calls.iter().filter(|(_, p)| p == "/props").count(), 2);
    assert_eq!(
        calls.iter().filter(|(_, p)| p == "/apply-template").count(),
        1
    );
    assert!(calls.iter().all(|(_, p)| p != "/v1/chat/completions"));
    let mut new_instance = false;
    inspection_support(
        RuntimeProfile::B11429,
        &mut new_instance,
        |_, path, body| {
            calls.push(("new".into(), path.into()));
            Ok(if path == "/props" {
                caps(None)
            } else {
                render(body.unwrap(), false)
            })
        },
    )
    .unwrap();
    assert_eq!(
        calls.iter().filter(|(_, p)| p == "/apply-template").count(),
        2
    );
    assert!(
        inspection_support(RuntimeProfile::B11429, &mut cached, |_, _, _| {
            Ok(caps(Some(json!(false))))
        })
        .is_err()
    );
    assert!(!cached);
}

#[test]
fn cached_success_does_not_grant_unknown_profiles_or_null_third() {
    for profile in [RuntimeProfile::Unknown, RuntimeProfile::B11429] {
        let mut cache = true;
        assert!(inspection_support(profile, &mut cache, |_, _, _| {
            Ok(if profile == RuntimeProfile::Unknown {
                caps(None)
            } else {
                caps(Some(Value::Null))
            })
        })
        .is_err());
        assert!(!cache);
    }
}

#[test]
fn failed_or_cancelled_expired_or_malformed_probe_never_caches_or_generates() {
    for code in [
        "RUNNER_CANCELLED",
        "PROVIDER_TIMEOUT",
        "PROVIDER_PROTOCOL",
        "UNSUPPORTED",
    ] {
        let mut cached = false;
        let mut paths = Vec::new();
        let outcome = inspection_support(RuntimeProfile::B11429, &mut cached, |_, path, _| {
            paths.push(path.to_owned());
            if path == "/props" {
                Ok(caps(None))
            } else {
                Err(error(code, "Synthetic failure"))
            }
        });
        assert_eq!(outcome.unwrap_err().code, code);
        assert!(!cached);
        assert_eq!(paths, ["/props", "/apply-template"]);
    }
    let mut cached = false;
    assert!(
        inspection_support(RuntimeProfile::B11429, &mut cached, |_, path, _| {
            Ok(if path == "/props" {
                caps(None)
            } else {
                json!({"prompt":"lost evidence"})
            })
        })
        .is_err()
    );
    assert!(!cached);
}

#[test]
fn loader_hash_or_self_reported_version_alone_cannot_identify_profile() {
    let mut inventory = BTreeMap::new();
    inventory.insert(
        "llama-server".into(),
        (
            0o755,
            "5faf410bbbc00a1b94b09ee01623699f255365628dfa73b1ee552d9e52a2231d".into(),
        ),
    );
    assert_eq!(
        runtime_profile(&inventory, Backend::Cpu),
        RuntimeProfile::Unknown
    );
    inventory.insert("version".into(), (0o644, "b11429/d81235049".into()));
    assert_eq!(
        runtime_profile(&inventory, Backend::Cpu),
        RuntimeProfile::Unknown
    );
}

fn packaged_inventory(vulkan: bool) -> BTreeMap<String, (u32, String)> {
    // Exact original managed manifest, captured before this correction and bound
    // to the independently verified package—not a regenerated expected inventory.
    let bytes = include_bytes!("../tests/fixtures/runtime-b11429-release-manifest.json");
    assert_eq!(
        filesystem::digest(bytes),
        "5b3a8571fa3a77e604a78b2a6872d87b6ec6793d398c58efce417ce7fd525ef2"
    );
    let manifest: ReleaseManifest = serde_json::from_slice(bytes).unwrap();
    assert_eq!(manifest.schema, 1);
    let prefix = format!(
        "smith_backend/runtime/{}/",
        if vulkan { "vulkan" } else { "cpu" }
    );
    let entries: BTreeMap<_, _> = manifest
        .entries
        .into_iter()
        .filter_map(|(path, entry)| {
            path.strip_prefix(&prefix).map(|name| {
                assert!(!name.is_empty() && !name.contains('/'));
                assert_eq!(entry.kind, "file");
                (name.to_owned(), (entry.mode, entry.sha256.unwrap()))
            })
        })
        .collect();
    assert_eq!(entries.len(), if vulkan { 26 } else { 25 });
    entries
}

#[test]
fn original_manifest_requires_packaged_0755_libraries_not_old_0644_fixture() {
    for (vulkan, backend, libraries) in [(false, Backend::Cpu, 21), (true, Backend::Vulkan, 22)] {
        let original = packaged_inventory(vulkan);
        let mut incorrect = original.clone();
        let mut changed = 0;
        for (name, (mode, _)) in &mut incorrect {
            if name.starts_with("lib") {
                assert_eq!(*mode, 0o755);
                *mode = 0o644;
                changed += 1;
            } else {
                assert_eq!(*mode, if name == "llama-server" { 0o755 } else { 0o644 });
            }
        }
        assert_eq!(changed, libraries);
        assert_eq!(runtime_profile(&original, backend), RuntimeProfile::B11429);
        assert_eq!(
            runtime_profile(&incorrect, backend),
            RuntimeProfile::Unknown
        );
    }
}

#[test]
fn profile_requires_complete_pinned_inventory_including_modes_and_libraries() {
    for vulkan in [false, true] {
        let backend = if vulkan {
            Backend::Vulkan
        } else {
            Backend::Cpu
        };
        let other = if vulkan {
            Backend::Cpu
        } else {
            Backend::Vulkan
        };
        let original = packaged_inventory(vulkan);
        assert_eq!(runtime_profile(&original, backend), RuntimeProfile::B11429);
        assert_eq!(runtime_profile(&original, other), RuntimeProfile::Unknown);
        for name in original.keys() {
            let mut altered = original.clone();
            altered.get_mut(name).unwrap().1 = "0".repeat(64);
            assert_eq!(runtime_profile(&altered, backend), RuntimeProfile::Unknown);
            let mut altered = original.clone();
            altered.get_mut(name).unwrap().0 ^= 0o111;
            assert_eq!(runtime_profile(&altered, backend), RuntimeProfile::Unknown);
            let mut altered = original.clone();
            altered.remove(name);
            assert_eq!(runtime_profile(&altered, backend), RuntimeProfile::Unknown);
        }
        let mut altered = original;
        altered.insert("extra".into(), (0o644, "a".repeat(64)));
        assert_eq!(runtime_profile(&altered, backend), RuntimeProfile::Unknown);
    }
}

#[test]
fn runner_failure_and_shutdown_clear_instance_success_even_without_a_child() {
    let mut runner = Runner {
        child: None,
        pid: 0,
        dir: None,
        socket: PathBuf::new(),
        key: String::new(),
        alias: "fixture".into(),
        context: CONTEXT,
        lock: None,
        lease_return: None,
        stderr: Arc::new(Mutex::new(Vec::new())),
        evidence_stderr: Arc::new(Mutex::new(StartupEvidence::default())),
        cancelled: Arc::new(|| false),
        runtime_profile: RuntimeProfile::B11429,
        inspection_template_verified: true,
    };
    runner.fail();
    assert!(!runner.inspection_template_verified);
    runner.inspection_template_verified = true;
    runner.shutdown().unwrap();
    assert!(!runner.inspection_template_verified);
}

#[test]
fn verified_profile_cannot_be_reused_for_a_different_executable_path() {
    let verified = VerifiedRuntime {
        executable: PathBuf::from("verified-runtime"),
        backend: Backend::Cpu,
        profile: RuntimeProfile::B11429,
    };
    let mut config = RunnerConfig {
        executable: verified.executable.clone(),
        runtime_parent: PathBuf::new(),
        lock: PathBuf::new(),
        context: CONTEXT,
        threads: 2,
        backend: Backend::Cpu,
        verified_runtime: Some(verified),
    };
    assert_eq!(config.verified_profile(), RuntimeProfile::B11429);
    config.backend = Backend::Vulkan;
    assert_eq!(config.verified_profile(), RuntimeProfile::Unknown);
    config.backend = Backend::Cpu;
    config.executable = PathBuf::from("different-runtime");
    assert_eq!(config.verified_profile(), RuntimeProfile::Unknown);
    config.verified_runtime = None;
    assert_eq!(config.verified_profile(), RuntimeProfile::Unknown);
}

#[test]
fn actual_request_template_and_context_guards_remain_exact() {
    let probe = InspectionProbe::new().unwrap();
    assert_eq!(
        inspection_template(&json!({"messages":probe.request["messages"],
        "tools":probe.request["tools"],"tool_choice":"auto","parallel_tool_calls":false})),
        probe.request
    );
    let budget = (CONTEXT - MAX_TOKENS - TOKEN_MARGIN) as usize;
    assert!(check_inspection_tokens(&json!({"tokens":vec![1;budget]}), CONTEXT).is_ok());
    assert_eq!(
        check_inspection_tokens(&json!({"tokens":vec![1;budget+1]}), CONTEXT)
            .unwrap_err()
            .code,
        "CONTEXT_LIMIT"
    );
}

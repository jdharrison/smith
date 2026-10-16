//! Frozen vendor fixtures: parsing and request construction without any transport.
use serde_json::{json, Value};
use smith_service::app::{Message, MAX_CONTEXT, MAX_TEXT};
use smith_service::cloud::{
    self, build_body, parse_response, HttpResponse, Vendor, BODY_LIMIT, HEADER_LIMIT, MODELS,
};
fn openai(model: &str) -> Value {
    json!({"object":"response","model":model,"status":"completed","error":null,"incomplete_details":null,"output":[{"type":"reasoning","summary":["reasoning-canary"],"encrypted_content":"encrypted-canary"},{"type":"message","role":"assistant","status":"completed","phase":"commentary","content":[{"type":"output_text","text":"commentary-canary"}]},{"type":"message","role":"assistant","status":"completed","phase":"final_answer","content":[{"type":"output_text","text":"Hello","annotations":[{"url":"https://untrusted.invalid"}]},{"type":"output_text","text":"."}]}]})
}
fn anthropic(model: &str) -> Value {
    json!({"type":"message","role":"assistant","model":model,"stop_reason":"end_turn","stop_sequence":null,"content":[{"type":"thinking","thinking":"thinking-canary","signature":"signature-canary"},{"type":"redacted_thinking","data":"redacted-canary"},{"type":"text","text":"Hello"},{"type":"text","text":"."}]})
}
fn parse(v: Vendor, model: &str, value: &Value) -> Result<String, smith_service::app::AppError> {
    parse_response(
        v,
        model,
        &HttpResponse {
            status: 200,
            header_bytes: 100,
            body: serde_json::to_vec(value).unwrap(),
        },
    )
}
#[test]
fn frozen_catalog_exact_identity_order_context_and_provider_bounds() {
    let expected = [
        (Vendor::Openai, "gpt-6-astra", "GPT-6 Astra", 1_050_000),
        (Vendor::Openai, "gpt-6-sol", "GPT-6 Sol", 1_050_000),
        (Vendor::Openai, "gpt-6.1-sol", "GPT-6.1 Sol", 1_050_000),
        (Vendor::Openai, "gpt-6-luna", "GPT-6 Luna", 1_050_000),
        (Vendor::Openai, "gpt-5.6-sol", "GPT-5.6 Sol", 1_050_000),
        (Vendor::Openai, "gpt-5.6-terra", "GPT-5.6 Terra", 1_050_000),
        (Vendor::Openai, "gpt-5.6-luna", "GPT-5.6 Luna", 1_050_000),
        (
            Vendor::Anthropic,
            "claude-sonnet-5-5",
            "Claude Sonnet 5.5",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-haiku-5-5",
            "Claude Haiku 5.5",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-opus-5-5",
            "Claude Opus 5.5",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-fable-5-1",
            "Claude Fable 5.1",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-opus-5",
            "Claude Opus 5",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-opus-4-8",
            "Claude Opus 4.8",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-opus-4-7",
            "Claude Opus 4.7",
            1_000_000,
        ),
        (
            Vendor::Anthropic,
            "claude-sonnet-5",
            "Claude Sonnet 5",
            1_000_000,
        ),
    ];
    let actual: Vec<_> = MODELS
        .iter()
        .map(|m| (m.vendor, m.id, m.label, m.context))
        .collect();
    assert_eq!(actual.as_slice(), expected.as_slice());
    let identities: std::collections::HashSet<_> =
        MODELS.iter().map(|m| (m.vendor, m.id)).collect();
    assert_eq!(identities.len(), MODELS.len());
    let counts = Vendor::ALL.map(|vendor| MODELS.iter().filter(|m| m.vendor == vendor).count());
    assert_eq!(counts, [7, 8]);
    assert!(counts.iter().all(|count| *count <= 8));
    assert!(256 + MODELS.len() <= 272);
}
#[test]
fn frozen_catalog_exact_requests_and_final_only_followup() {
    for m in MODELS {
        let messages = vec![
            Message {
                role: "user".into(),
                content: "before".into(),
            },
            Message {
                role: "assistant".into(),
                content: "final only".into(),
            },
            Message {
                role: "user".into(),
                content: "after".into(),
            },
        ];
        let b: Value =
            serde_json::from_slice(&build_body(m.vendor, m.id, &messages).unwrap()).unwrap();
        assert_eq!(b["model"], m.id);
        match m.vendor {
            Vendor::Openai => {
                assert_eq!(b.as_object().unwrap().len(), 7);
                assert_eq!(b["store"], false);
                assert_eq!(b["stream"], false);
                assert_eq!(b["background"], false);
                assert_eq!(b["reasoning"], json!({"effort":"low"}));
                assert_eq!(b["max_output_tokens"], 8192);
                assert_eq!(
                    b["input"][1],
                    json!({"role":"assistant","content":"final only","phase":"final_answer"})
                );
                assert_eq!(parse(m.vendor, m.id, &openai(m.id)).unwrap(), "Hello.");
            }
            Vendor::Anthropic => {
                assert_eq!(b.as_object().unwrap().len(), 4);
                assert_eq!(b["max_tokens"], 8192);
                assert_eq!(b["stream"], false);
                assert_eq!(
                    b["messages"][1],
                    json!({"role":"assistant","content":[{"type":"text","text":"final only"}]})
                );
                assert_eq!(parse(m.vendor, m.id, &anthropic(m.id)).unwrap(), "Hello.");
            }
        }
    }
}
#[test]
fn exact_response_identity_and_required_fields() {
    for m in MODELS {
        let good = if m.vendor == Vendor::Openai {
            openai(m.id)
        } else {
            anthropic(m.id)
        };
        for bad in [
            format!("{}-2026-10-15", m.id),
            m.id.to_uppercase(),
            format!("{} ", m.id),
            "other".into(),
        ] {
            let mut v = good.clone();
            v["model"] = json!(bad);
            assert!(parse(m.vendor, m.id, &v).is_err());
        }
        for other in MODELS.iter().filter(|other| other.id != m.id) {
            let mut v = good.clone();
            v["model"] = json!(other.id);
            assert_eq!(
                parse(m.vendor, m.id, &v).unwrap_err().code,
                "CLOUD_PROTOCOL"
            );
        }
        let keys = if m.vendor == Vendor::Openai {
            vec![
                "object",
                "model",
                "status",
                "error",
                "incomplete_details",
                "output",
            ]
        } else {
            vec![
                "type",
                "role",
                "model",
                "stop_reason",
                "stop_sequence",
                "content",
            ]
        };
        for key in keys {
            let mut v = good.clone();
            v.as_object_mut().unwrap().remove(key);
            assert!(parse(m.vendor, m.id, &v).is_err(), "missing {key}");
            let mut v = good.clone();
            v[key] = json!(42);
            assert!(parse(m.vendor, m.id, &v).is_err(), "wrong type {key}");
        }
    }
}
#[test]
fn catalog_lookup_and_requests_reject_unknown_and_wrong_provider_models() {
    let messages = [Message {
        role: "user".into(),
        content: "hello".into(),
    }];
    for m in MODELS {
        assert_eq!(cloud::model(m.vendor, m.id).unwrap().id, m.id);
        let other = if m.vendor == Vendor::Openai {
            Vendor::Anthropic
        } else {
            Vendor::Openai
        };
        assert!(cloud::model(other, m.id).is_none());
        assert!(build_body(other, m.id, &messages).is_err());
        for invalid in [
            format!("{}-2026-10-15", m.id),
            m.id.to_uppercase(),
            format!("{} ", m.id),
        ] {
            assert!(cloud::model(m.vendor, &invalid).is_none());
            assert!(build_body(m.vendor, &invalid, &messages).is_err());
        }
    }
    for vendor in Vendor::ALL {
        for unknown in ["invented", "gpt-5.6", "gpt6.1.sol", "claude5.5"] {
            assert!(build_body(vendor, unknown, &messages).is_err());
        }
    }
}
#[test]
fn openai_all_noncompleted_states_and_truncation_reject_partial_text() {
    for status in [
        "incomplete",
        "failed",
        "cancelled",
        "queued",
        "in_progress",
        "unknown",
        "",
    ] {
        let mut v = openai("gpt-6-astra");
        v["status"] = json!(status);
        assert!(parse(Vendor::Openai, "gpt-6-astra", &v).is_err());
    }
    let mut v = openai("gpt-6-astra");
    v["status"] = json!("incomplete");
    v["incomplete_details"] = json!({"reason":"max_output_tokens"});
    assert_eq!(
        parse(Vendor::Openai, "gpt-6-astra", &v).unwrap_err().code,
        "MODEL_TRUNCATED"
    );
}
#[test]
fn openai_rejects_tools_refusals_unknown_phases_and_remote_state_anywhere() {
    let good = openai("gpt-6-astra");
    for kind in [
        "function_call",
        "web_search_call",
        "tool_call",
        "compaction",
        "unknown",
        "refusal",
    ] {
        let mut v = good.clone();
        v["output"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":kind}));
        assert!(parse(Vendor::Openai, "gpt-6-astra", &v).is_err());
    }
    for phase in ["commentary", "final_answer"] {
        let mut v = good.clone();
        v["output"][1]["phase"] = json!(phase);
        v["output"][1]["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"refusal","refusal":"unsafe"}));
        assert!(parse(Vendor::Openai, "gpt-6-astra", &v).is_err());
    }
    for (key, value) in [
        ("store", json!(true)),
        ("background", json!(true)),
        ("conversation", json!("id")),
        ("previous_response_id", json!("id")),
    ] {
        let mut v = good.clone();
        v[key] = value;
        assert!(parse(Vendor::Openai, "gpt-6-astra", &v).is_err());
    }
    for (pointer, value) in [
        ("/output/2/phase", json!("unknown")),
        ("/output/2/role", json!("user")),
        ("/output/2/status", json!("in_progress")),
        ("/output/2/content/0/text", json!(null)),
        ("/output/0/status", json!(false)),
    ] {
        let mut v = good.clone();
        if pointer == "/output/0/status" {
            v["output"][0]["status"] = value;
        } else {
            *v.pointer_mut(pointer).unwrap() = value;
        }
        assert!(parse(Vendor::Openai, "gpt-6-astra", &v).is_err());
    }
}
#[test]
fn anthropic_rejects_all_nonfinal_stops_and_mixed_tool_blocks() {
    for stop in [
        "max_tokens",
        "model_context_window_exceeded",
        "refusal",
        "tool_use",
        "pause_turn",
        "stop_sequence",
        "unknown",
        "",
    ] {
        let mut v = anthropic("claude-sonnet-5-5");
        v["stop_reason"] = json!(stop);
        assert!(parse(Vendor::Anthropic, "claude-sonnet-5-5", &v).is_err());
    }
    for kind in [
        "tool_use",
        "server_tool_use",
        "tool_result",
        "image",
        "unknown",
    ] {
        let mut v = anthropic("claude-sonnet-5-5");
        v["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":kind}));
        assert!(parse(Vendor::Anthropic, "claude-sonnet-5-5", &v).is_err());
    }
}
#[test]
fn final_text_boundaries_reasoning_only_and_whitespace() {
    for m in MODELS {
        for bytes in [0, 1, MAX_CONTEXT, MAX_CONTEXT + 1] {
            let text = "a".repeat(bytes);
            let v = if m.vendor == Vendor::Openai {
                json!({"object":"response","model":m.id,"status":"completed","error":null,"incomplete_details":null,"output":[{"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":text}]}]})
            } else {
                json!({"type":"message","role":"assistant","model":m.id,"stop_reason":"end_turn","stop_sequence":null,"content":[{"type":"text","text":text}]})
            };
            assert_eq!(
                parse(m.vendor, m.id, &v).is_ok(),
                bytes > 0 && bytes <= MAX_CONTEXT
            );
        }
        let mut v = if m.vendor == Vendor::Openai {
            openai(m.id)
        } else {
            anthropic(m.id)
        };
        if m.vendor == Vendor::Openai {
            v["output"].as_array_mut().unwrap().truncate(1);
        } else {
            v["content"].as_array_mut().unwrap().truncate(2);
        }
        assert!(parse(m.vendor, m.id, &v).is_err());
    }
}
#[test]
fn duplicate_discriminants_invalid_json_and_utf8_are_rejected() {
    for bytes in [
        b"{\"model\":\"wrong\",\"model\":\"gpt-6-astra\"}".to_vec(),
        b"{\"output\":[{\"type\":\"tool\",\"type\":\"message\"}]}".to_vec(),
        b"{\"extra\":{\"a\":1,\"a\":2}}".to_vec(),
        vec![0xff],
        b"{} trailing".to_vec(),
    ] {
        assert!(parse_response(
            Vendor::Openai,
            "gpt-6-astra",
            &HttpResponse {
                status: 200,
                header_bytes: 0,
                body: bytes
            }
        )
        .is_err());
    }
}
#[test]
fn response_and_header_size_caps_apply_to_errors_too() {
    for status in [200, 401, 429, 503] {
        for (headers, body) in [(HEADER_LIMIT + 1, vec![]), (0, vec![b' '; BODY_LIMIT + 1])] {
            let r = parse_response(
                Vendor::Openai,
                "gpt-6-astra",
                &HttpResponse {
                    status,
                    header_bytes: headers,
                    body,
                },
            )
            .unwrap_err();
            assert_eq!(r.code, "CLOUD_PROTOCOL");
        }
    }
    let body = serde_json::to_vec(&openai("gpt-6-astra")).unwrap();
    assert!(parse_response(
        Vendor::Openai,
        "gpt-6-astra",
        &HttpResponse {
            status: 200,
            header_bytes: HEADER_LIMIT,
            body
        }
    )
    .is_ok());
}
#[test]
fn http_error_categories_never_expose_vendor_messages() {
    for (status, code) in [
        (301, "CLOUD_PROTOCOL"),
        (307, "CLOUD_PROTOCOL"),
        (202, "CLOUD_PROTOCOL"),
        (204, "CLOUD_PROTOCOL"),
        (400, "CLOUD_PROTOCOL"),
        (401, "CLOUD_AUTH"),
        (403, "CLOUD_AUTH"),
        (402, "CLOUD_QUOTA"),
        (429, "CLOUD_RATE_LIMIT"),
        (503, "CLOUD_UNAVAILABLE"),
        (529, "CLOUD_UNAVAILABLE"),
    ] {
        let r = parse_response(
            Vendor::Openai,
            "gpt-6-astra",
            &HttpResponse {
                status,
                header_bytes: 0,
                body: br#"{"error":{"message":"vendor-secret-canary"}}"#.to_vec(),
            },
        )
        .unwrap_err();
        assert_eq!(r.code, code);
        assert!(!r.message.contains("vendor-secret-canary"));
    }
    let r = parse_response(
        Vendor::Openai,
        "gpt-6-astra",
        &HttpResponse {
            status: 429,
            header_bytes: 0,
            body: br#"{"error":{"code":"credit_balance_exhausted","message":"private"}}"#.to_vec(),
        },
    )
    .unwrap_err();
    assert_eq!(r.code, "CLOUD_QUOTA");
}
#[test]
fn deterministic_request_context_drops_only_oldest_complete_pairs() {
    let history: Vec<_> = (0..16)
        .flat_map(|i| {
            [
                Message {
                    role: "user".into(),
                    content: format!("{i:04}{}", "u".repeat(1020)),
                },
                Message {
                    role: "assistant".into(),
                    content: "a".repeat(3072),
                },
            ]
        })
        .collect();
    let current = "\u{0001}".repeat(MAX_TEXT);
    let context = cloud::context(Vendor::Openai, "gpt-6-astra", &history, &current).unwrap();
    assert_eq!(history.len(), 32);
    assert_eq!(context.len(), 25);
    assert!(context[0].content.starts_with("0004"));
    assert_eq!(context.last().unwrap().content, current);
    assert!(
        build_body(Vendor::Openai, "gpt-6-astra", &context)
            .unwrap()
            .len()
            <= BODY_LIMIT
    );
    assert!(cloud::context(
        Vendor::Openai,
        "gpt-6-astra",
        &history,
        &"x".repeat(MAX_TEXT + 1)
    )
    .is_err());
    assert!(build_body(Vendor::Openai, "gpt6.0", &context).is_err());
}

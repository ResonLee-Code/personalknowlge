//! 决策模型客户端（D4）契约测试：用本机桩服务冒充 Jev / Perplexity Decisions 端点。
//!
//! 覆盖：
//! 1. **请求形状**与官方文档一致（路径、Bearer 头、`model`、`questions[<id>]` 的类型与 criteria）；
//! 2. `score` / `noul` 答案的解析与映射（归一化、冲突判定）；
//! 3. **错误映射**：401 / 429（带 Retry-After）都说人话，且**绝不含 Key**；
//! 4. **批量拆分**：候选超过上限时分多次请求。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use personal_knowledge_os_lib::application::ai::policy::DecisionConfig;
use personal_knowledge_os_lib::application::ai::system_one::SystemOneDecisionModel;
use personal_knowledge_os_lib::application::ai::traits::{
    DecisionCandidate, DecisionChoice, DecisionKind, DecisionModel, DecisionState,
};

struct Stub {
    endpoint: String,
    calls: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<serde_json::Value>>>,
    auth_headers: Arc<Mutex<Vec<String>>>,
    paths: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// 起一个桩端点：`responder` 收到解析后的请求体，回 `(状态码, 响应体, 额外响应头)`。
fn spawn_stub<F>(responder: F) -> Stub
where
    F: Fn(&serde_json::Value) -> (u16, String, Vec<(String, String)>) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    let calls = Arc::new(AtomicUsize::new(0));
    let bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let auth_headers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let paths: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let (counter, seen_bodies, seen_auth, seen_paths) = (
        Arc::clone(&calls),
        Arc::clone(&bodies),
        Arc::clone(&auth_headers),
        Arc::clone(&paths),
    );
    let responder = Arc::new(responder);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let responder = Arc::clone(&responder);
            let _ = handle(&mut stream, responder.as_ref(), &seen_bodies, &seen_auth, &seen_paths);
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    Stub { endpoint: format!("http://127.0.0.1:{port}"), calls, bodies, auth_headers, paths }
}

fn handle<F>(
    stream: &mut TcpStream,
    responder: &F,
    bodies: &Arc<Mutex<Vec<serde_json::Value>>>,
    auth: &Arc<Mutex<Vec<String>>>,
    paths: &Arc<Mutex<Vec<String>>>,
) -> std::io::Result<()>
where
    F: Fn(&serde_json::Value) -> (u16, String, Vec<(String, String)>),
{
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    paths
        .lock()
        .expect("锁")
        .push(request_line.split_whitespace().nth(1).unwrap_or("").to_string());
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
        let lowered = header.to_ascii_lowercase();
        if let Some(value) = lowered.strip_prefix("authorization:") {
            auth.lock().expect("锁").push(value.trim().to_string());
        }
        if let Some(value) = lowered.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 {
        reader.read_exact(&mut body)?;
    }
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));
    bodies.lock().expect("锁").push(parsed.clone());

    let (status, payload, extra_headers) = responder(&parsed);
    let reason = if status == 200 { "OK" } else { "Error" };
    let extra: String = extra_headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect();
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nx-request-id: test-request-id\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.as_bytes().len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn model(endpoint: &str) -> SystemOneDecisionModel {
    SystemOneDecisionModel::new(&DecisionConfig {
        base_url: endpoint.to_string(),
        model: "pplx-decider-v1.1-27b".to_string(),
        key: "pplx-secret-key".to_string(),
        proxy: None,
    })
}

fn state(count: usize) -> DecisionState {
    DecisionState {
        query: "Rust 异步调度".to_string(),
        candidates: (0..count)
            .map(|index| DecisionCandidate {
                note_id: format!("note_{index}"),
                title: format!("候选 {index}"),
                snippet: "任务唤醒与批量窃取。".to_string(),
                updated_at: "2026-10-13T10:00:00+08:00".to_string(),
                source_reliability: Some(4),
                valid_until: None,
            })
            .collect(),
    }
}

/// 桩：按请求里的 question id 原样回 `score` 答案（固定最高档）。
fn score_stub() -> impl Fn(&serde_json::Value) -> (u16, String, Vec<(String, String)>)
       + Send
       + Sync
       + 'static {
    |body: &serde_json::Value| {
        let mut answers = serde_json::Map::new();
        if let Some(questions) = body.get("questions").and_then(|value| value.as_object()) {
            for id in questions.keys() {
                answers.insert(
                    id.clone(),
                    serde_json::json!({
                        "type": "score",
                        "score": 2.0,
                        "confidence": 0.9,
                        "legend": { "0": "不相关", "1": "部分相关", "2": "直接相关" },
                        "probabilities": { "0": 0.0, "1": 0.05, "2": 0.95 }
                    }),
                );
            }
        }
        (
            200,
            serde_json::json!({
                "model": "pplx-decider-v1.1-27b",
                "answers": answers,
                "usage": { "input_tokens": 120, "output_tokens": 4 }
            })
            .to_string(),
            Vec::new(),
        )
    }
}

#[test]
fn decide_sends_the_documented_shape_and_maps_the_score() {
    let stub = spawn_stub(score_stub());
    let model = model(&stub.endpoint);

    let results = model.decide(DecisionKind::Relevance, &state(2)).expect("决策");

    assert_eq!(results.len(), 2, "两条候选各一条结果");
    let first = &results[0];
    assert_eq!(first.note_id, "note_0");
    assert_eq!(first.score, 1.0, "档位加权分 2.0 → 归一化 1.0");
    assert_eq!(first.choice, DecisionChoice::Use, "最高档 → 使用");
    assert!((first.confidence - 0.9).abs() < 1e-6, "confidence 原样带回");
    assert!(
        first.reason.contains("档位加权分"),
        "理由要如实标注是我们折算的：{}",
        first.reason
    );

    assert_eq!(stub.paths.lock().unwrap().first().map(String::as_str), Some("/v1/decisions"));
    assert_eq!(
        stub.auth_headers.lock().unwrap().first().map(String::as_str),
        Some("bearer pplx-secret-key")
    );
    let body = stub.bodies.lock().unwrap().first().cloned().expect("请求体");
    assert_eq!(body["model"], "pplx-decider-v1.1-27b");
    assert_eq!(body["questions"]["note_0"]["type"], "score");
    assert_eq!(body["questions"]["note_0"]["criteria"][0], "不相关", "档位是有序数组，索引即分数");
    assert_eq!(body["state"]["candidates"][0]["id"], "note_0");
    assert_eq!(body["state"]["query"], "Rust 异步调度");
}

#[test]
fn judge_conflict_asks_a_noul_question_and_thresholds_at_half() {
    let stub = spawn_stub(|body: &serde_json::Value| {
        let id = body["questions"]
            .as_object()
            .and_then(|questions| questions.keys().next().cloned())
            .unwrap_or_default();
        (
            200,
            serde_json::json!({
                "model": "pplx-decider-v1.1-27b",
                "answers": { id: { "type": "noul", "noul": 0.93 } },
                "usage": { "input_tokens": 90, "output_tokens": 1 }
            })
            .to_string(),
            Vec::new(),
        )
    });
    let model = model(&stub.endpoint);

    let candidates = state(2).candidates;
    let verdict = model
        .judge_conflict("Rust 异步调度", &candidates)
        .expect("冲突判定")
        .expect("两条材料应当有结论");
    assert!(verdict.conflict, "noul=0.93 ≥ 0.5 应判冲突");
    assert!(verdict.reason.contains("0.93"), "理由里要带上概率：{}", verdict.reason);

    let body = stub.bodies.lock().unwrap().first().cloned().expect("请求体");
    assert_eq!(body["questions"]["conflict"]["type"], "noul");
    assert_eq!(body["state"]["materials"][0]["id"], "note_0");
}

#[test]
fn batching_splits_candidates_over_the_question_limit() {
    let stub = spawn_stub(score_stub());
    let model = model(&stub.endpoint);

    let results = model.decide(DecisionKind::Relevance, &state(125)).expect("决策");
    assert_eq!(results.len(), 125);
    assert!(stub.calls() >= 2, "超过 128 个问题上限要分批：实际 {} 次", stub.calls());
}

#[test]
fn unauthorized_and_rate_limited_errors_are_explained_without_leaking_the_key() {
    let stub = spawn_stub(|_| {
        (
            401,
            serde_json::json!({ "error": { "message": "Invalid API key", "type": "invalid_request_error" } })
                .to_string(),
            Vec::new(),
        )
    });
    let error = model(&stub.endpoint)
        .decide(DecisionKind::Relevance, &state(1))
        .expect_err("401 必须报错");
    assert!(error.message.contains("API Key"), "{}", error.message);
    assert!(error.message.contains("Authorization"), "要提示 Key 的放法：{}", error.message);
    assert!(!error.message.contains("pplx-secret-key"), "绝不能回显 Key：{}", error.message);
    assert!(
        error.message.contains("test-request-id"),
        "要带上 request-id 便于报障：{}",
        error.message
    );

    let limited = spawn_stub(|_| {
        (
            429,
            serde_json::json!({ "error": { "message": "Rate limit exceeded", "type": "rate_limit_error" } })
                .to_string(),
            vec![("Retry-After".to_string(), "7".to_string())],
        )
    });
    let error = model(&limited.endpoint)
        .decide(DecisionKind::Relevance, &state(1))
        .expect_err("429 必须报错");
    assert!(error.message.contains("限流"), "{}", error.message);
    assert!(error.message.contains('7'), "要提示 Retry-After 秒数：{}", error.message);
    assert!(!error.message.contains("pplx-secret-key"));
}

#[test]
fn endpoint_path_switches_between_perplexity_and_jev() {
    assert_eq!(SystemOneDecisionModel::path_for("https://api.perplexity.ai"), "/v1/decisions");
    assert_eq!(SystemOneDecisionModel::path_for("https://api.typesafe.ai"), "/v1/systemone");
}

/// 真机冒烟（**默认忽略**）：有 Key 时用一次真实请求验证"应用侧也能打通"。
///
/// ```bash
/// PKOS_LIVE_DECISION_KEY=pplx-xxx \
///   [https_proxy=http://127.0.0.1:8118] \
///   cargo test --offline --test decision_provider_flow live_decision_call -- --ignored --nocapture
/// ```
///
/// 只读：不碰知识库，只发一条最小问题。
#[test]
#[ignore = "真机冒烟：需要 PKOS_LIVE_DECISION_KEY（可选 https_proxy）"]
fn live_decision_call_proves_the_app_side_path() {
    let Ok(key) = std::env::var("PKOS_LIVE_DECISION_KEY") else {
        eprintln!("跳过：未设置 PKOS_LIVE_DECISION_KEY");
        return;
    };
    let base = std::env::var("PKOS_LIVE_DECISION_BASE")
        .unwrap_or_else(|_| "https://api.perplexity.ai".to_string());
    let model = std::env::var("PKOS_LIVE_DECISION_MODEL")
        .unwrap_or_else(|_| "pplx-decider-v1.1-27b".to_string());
    let decision_model = SystemOneDecisionModel::new(&DecisionConfig {
        base_url: base.clone(),
        model: model.clone(),
        key,
        proxy: None, // 交给环境变量 / 系统代理
    });

    let candidates = vec![
        DecisionCandidate {
            note_id: "c1".to_string(),
            title: "Rust 异步调度实现要点".to_string(),
            snippet: "实现要点在于任务唤醒与批量窃取。".to_string(),
            updated_at: "2026-10-13T10:00:00+08:00".to_string(),
            source_reliability: Some(4),
            valid_until: None,
        },
        DecisionCandidate {
            note_id: "c2".to_string(),
            title: "周末爬山记录".to_string(),
            snippet: "山上风很大，带了热水。".to_string(),
            updated_at: "2026-10-13T10:00:00+08:00".to_string(),
            source_reliability: Some(4),
            valid_until: None,
        },
    ];
    let state = DecisionState { query: "Rust 异步调度".to_string(), candidates };

    let results = decision_model
        .decide(DecisionKind::Relevance, &state)
        .expect("真机调用应当成功（若失败，先看 `出站代理` 设置）");
    println!("端点 {base} / 模型 {model}");
    for result in &results {
        println!(
            "  {} → score={:.3} confidence={:.3} choice={:?}",
            result.note_id, result.score, result.confidence, result.choice
        );
    }
    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|result| result.confidence > 0.0),
        "应当带回决策模型自己的置信度"
    );
}

/// 真机评测（**默认忽略**）：把服务里用的**同一份题库**打到真实端点上，
/// 算准确率 / ECE / Brier / 折外校准 / 自洽性 / 两次一致性。
///
/// ```bash
/// PKOS_LIVE_DECISION_KEY=pplx-xxx [https_proxy=http://127.0.0.1:8118] \
///   cargo test --offline --test decision_provider_flow live_decision_battery -- --ignored --nocapture
/// ```
#[test]
#[ignore = "真机评测：需要 PKOS_LIVE_DECISION_KEY（可选 https_proxy）"]
fn live_decision_battery_measures_calibration_on_our_own_items() {
    use personal_knowledge_os_lib::application::ai::calibration;
    use personal_knowledge_os_lib::application::ai::decision_probe;

    let Ok(key) = std::env::var("PKOS_LIVE_DECISION_KEY") else {
        eprintln!("跳过：未设置 PKOS_LIVE_DECISION_KEY");
        return;
    };
    let base = std::env::var("PKOS_LIVE_DECISION_BASE")
        .unwrap_or_else(|_| "https://api.perplexity.ai".to_string());
    let model_name = std::env::var("PKOS_LIVE_DECISION_MODEL")
        .unwrap_or_else(|_| "pplx-decider-v1.1-27b".to_string());
    let decision_model = SystemOneDecisionModel::new(&DecisionConfig {
        base_url: base.clone(),
        model: model_name.clone(),
        key,
        proxy: None,
    });

    let state = decision_probe::probe_state();
    let questions = decision_probe::probe_questions();
    let (first, tokens_a) = decision_model.raw_request(&state, &questions).expect("第一次");
    let (second, tokens_b) = decision_model.raw_request(&state, &questions).expect("第二次");

    let read = |value: &serde_json::Value, name: &str| -> Option<f32> {
        value.get("answers")?.get(name)?.get("noul")?.as_f64().map(|p| p as f32)
    };
    let mut samples: Vec<calibration::Sample> = Vec::new();
    let mut correct = 0usize;
    println!("题目\t\t期望\tP(是)\t判定");
    for spec in decision_probe::all_items() {
        let Some(p) = read(&first, spec.name) else { continue };
        match spec.expected_yes {
            Some(truth) => {
                let hit = (p > 0.5) == truth;
                if hit {
                    correct += 1;
                }
                samples.push((p, truth));
                println!(
                    "{:16}\t{:5}\t{p:.4}\t{}",
                    spec.name,
                    if truth { "Yes" } else { "No" },
                    if hit { "✓" } else { "✗" }
                );
            }
            None => println!("{:16}\t(观察)\t{p:.4}\t—", spec.name),
        }
    }
    let scored = samples.len();
    // 分族统计：混在一起算会被"题太清晰"那一族拉平，看不出"材料里没有答案时会不会装懂"。
    let split = |underdetermined_family: bool| -> (usize, usize, Vec<calibration::Sample>) {
        let mut correct_here = 0usize;
        let mut picked: Vec<calibration::Sample> = Vec::new();
        for spec in decision_probe::all_items() {
            if decision_probe::is_underdetermined(spec.name) != underdetermined_family {
                continue;
            }
            let Some(truth) = spec.expected_yes else { continue };
            let Some(p) = read(&first, spec.name) else { continue };
            if (p > 0.5) == truth {
                correct_here += 1;
            }
            picked.push((p, truth));
        }
        let total_here = picked.len();
        (correct_here, total_here, picked)
    };
    let (det_correct, det_total, det_samples) = split(false);
    let (und_correct, und_total, und_samples) = split(true);
    let calibration = calibration::evaluate(&samples);
    let honest = calibration::out_of_sample_ece(&samples);
    let deterministic = decision_probe::all_items().iter().all(|spec| {
        match (read(&first, spec.name), read(&second, spec.name)) {
            (Some(left), Some(right)) => (left - right).abs() < 1e-6,
            _ => true,
        }
    });
    let coherence = match (read(&first, "coherence_pos"), read(&first, "coherence_neg")) {
        (Some(left), Some(right)) => Some(left + right),
        _ => None,
    };
    let tokens = tokens_a + tokens_b;

    println!("\n===== 决策模型真机评测（我们自己的中文题） =====");
    println!("端点 {base} / 模型 {model_name}");
    println!("准确率        : {correct}/{scored} = {:.1}%", correct as f32 / scored as f32 * 100.0);
    println!(
        "  ├ 材料内（能推出来）: {det_correct}/{det_total} = {:.1}%  ECE {:.4}",
        det_correct as f32 / det_total.max(1) as f32 * 100.0,
        calibration::evaluate(&det_samples).ece
    );
    println!(
        "  └ 材料外（推不出来）: {und_correct}/{und_total} = {:.1}%  ECE {:.4}  平均陈述 {:.3}（理想 0.5）",
        und_correct as f32 / und_total.max(1) as f32 * 100.0,
        calibration::evaluate(&und_samples).ece,
        und_samples.iter().map(|(p, _)| *p).sum::<f32>() / und_total.max(1) as f32
    );
    println!("ECE（线 0.05）: {:.4}（Brier {:.4}）", calibration.ece, calibration.brier);
    println!(
        "折外校准 ECE  : {}（一半拟合、一半评估）",
        honest.map(|value| format!("{value:.4}")).unwrap_or_else(|| "样本不足".to_string())
    );
    println!("自洽性 P(A)+P(¬A): {}", coherence.map(|v| format!("{v:.4}")).unwrap_or_default());
    println!("两次一致性    : {}", if deterministic { "逐位相同" } else { "有差异" });
    println!("用量          : {tokens} 输入 token（两次）× $0.02/Mtok ≈ ${:.6}", tokens as f64 * 0.02 / 1e6);
    println!("分箱（说多少 vs 实际对多少）：");
    for bin in &calibration.bins {
        println!(
            "  {}  说 {:.3} → 实际 {:.3}（{} 条）",
            bin.range, bin.mean_stated, bin.actual_share, bin.count
        );
    }
    println!("===============================================");

    assert!(scored >= 20, "题库应当有足够多可判分的题：{scored}");

    // —— A/B：同一批打分任务上，规则档 vs 决策档（D4 收尾）——
    use personal_knowledge_os_lib::application::ai::rule_provider::RuleDecisionProvider;
    let rule = RuleDecisionProvider::default();
    let rule_outcome = decision_probe::evaluate_ab(&rule, decision_probe::AB_CASES);
    let decision_outcome = decision_probe::evaluate_ab(&decision_model, decision_probe::AB_CASES);
    println!(
        "\n===== A/B：打分任务（{} 条，相关/不相关各半，人工标注）=====",
        decision_probe::AB_CASES.len()
    );
    for outcome in [&rule_outcome, &decision_outcome] {
        println!(
            "  {:28} 准确率 {}/{} = {:.1}%  相关均分 {:.3}  不相关均分 {:.3}  区分度 {:+.3}",
            outcome.provider,
            outcome.correct,
            outcome.total,
            outcome.accuracy * 100.0,
            outcome.mean_relevant,
            outcome.mean_irrelevant,
            outcome.separation
        );
    }
    println!("===============================================");
}

//! 生成模型候选链：在线（已授权且确认）→ 本地。
//!
//! 模板 / 规则兜底由具体能力实现。这里不执行生成，只负责把“哪些档位可以构造”
//! 集中在一处，避免 Ask、Organize 各自复制隐私闸门逻辑。

use std::sync::Arc;

use crate::application::context::WorkspaceContext;

use super::local_model::OllamaModel;
use super::policy::AiPolicyService;
use super::remote_model::RemoteModel;
use super::traits::GenerativeModel;

pub type GenerationCandidate = (String, String, bool, Arc<dyn GenerativeModel>);

pub fn generation_candidates(
    ctx: &Arc<WorkspaceContext>,
    capability: &str,
    confirm_outbound: bool,
    warnings: &mut Vec<String>,
) -> Vec<GenerationCandidate> {
    let policy = ctx.settings().ai;
    let service = AiPolicyService::new(Arc::clone(ctx));
    let mut candidates: Vec<GenerationCandidate> = Vec::new();
    if policy.remote_model_compose {
        if service.ensure_remote_model_allowed(capability).is_ok() {
            if service.outbound_confirmed(confirm_outbound) {
                if let Some((base, model, key)) = service.remote_model() {
                    candidates.push((
                        RemoteModel::provider_id(&model),
                        "online".to_string(),
                        false,
                        Arc::new(
                            RemoteModel::new(&base, &model, &key)
                                .with_proxy(service.outbound_proxy()),
                        ),
                    ));
                }
            } else {
                let warning = "在线模型本次未获确认，已在零请求状态下跳过。".to_string();
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
                service.audit(capability, false, "denied", 0, Some("OUTBOUND_UNCONFIRMED"));
            }
        } else {
            warnings
                .push("在线模型已启用但未配置完整或未授权，本次只使用本地 / 模板档。".to_string());
        }
    }
    if policy.local_model_compose {
        if let Some((endpoint, model)) = service.local_model() {
            candidates.push((
                OllamaModel::provider_id(&model),
                "local".to_string(),
                true,
                Arc::new(OllamaModel::new(&endpoint, &model)),
            ));
        }
    }
    candidates
}

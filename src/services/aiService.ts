import type {
  AiAuditEntry,
  AiDiagnosis,
  AiPolicy,
  ClippedPage,
  DecisionProbeReport,
  KbHit,
  LocalModelPreset,
  RetrievalReport,
  VectorIndexPreview,
  VectorIndexReport,
  VectorIndexStatus,
  UpdateAiPolicyInput,
  WebSearchHit,
} from '@/types';

import { call } from './api';

/**
 * AI 能力服务（V1.2.1 阶段 D1）。
 *
 * 闸门在后端（`application::ai::policy`）：默认「总开关关闭 + 联网仅本地」，
 * 这里只是转发，不做任何本地判断——避免前后端两套规则。
 */
export const aiService = {
  /** 当前策略（默认最保守） */
  policy: () => call<AiPolicy>('ai_policy_get'),

  /** 更新策略（总开关 / 联网档位 / 抓取正文 / 黑白名单 / 预算） */
  setPolicy: (input: UpdateAiPolicyInput) => call<AiPolicy>('ai_policy_set', { input }),

  /** 最近的行为审计（新的在前，不含正文） */
  auditTail: (limit = 20) => call<AiAuditEntry[]>('ai_audit_tail', { limit }),

  /** 本地模型的低配预设（值来自后端常量，前端不硬编码） */
  modelPresets: () => call<LocalModelPreset[]>('ai_model_presets'),

  /** 模型自检：现在「写报告」用哪一档、为什么（只读；只在需要时探测本机） */
  diagnose: () => call<AiDiagnosis>('ai_diagnose'),

  /**
   * 决策模型契约探针（D4）：用我们自己的中文题跑准确率 / 自洽性 / 两次一致性 /
   * 矛盾判别 / 耗时 / 用量。**只读**，不碰知识库。
   */
  decisionProbe: () => call<DecisionProbeReport>('ai_decision_probe'),

  /** 本地检索（复用 FTS5，不联网） */
  kbSearch: (keyword: string, limit = 8) => call<KbHit[]>('ai_kb_search', { keyword, limit }),

  /**
   * 多通道检索 + 融合（D3/N7）：返回**通道状态**（谁可用、为什么跳过）与融合命中。
   * 目前只有 `bm25` 一个通道；向量档接入后这里会多一条、并在不可用时自动跳过。
   */
  retrievalSearch: (keyword: string, limit = 8) =>
    call<RetrievalReport>('ai_retrieval_search', { keyword, limit }),

  /** 向量索引状态（D3）：建了多少、用的哪个模型、为什么不可用 */
  vectorIndexStatus: () => call<VectorIndexStatus>('ai_vector_index_status'),

  /** 构建**之前**的预览（要发多少、发给谁）；只读，不发请求 */
  vectorIndexPreview: () => call<VectorIndexPreview>('ai_vector_index_preview'),

  /** 增量构建向量索引；`confirm` 必须为 true（把片段发出去的一次性动作） */
  vectorIndexBuild: (confirm: boolean) =>
    call<VectorIndexReport>('ai_vector_index_build', { confirm }),

  /** 清掉本地向量（派生数据，随时可重建） */
  vectorIndexClear: () => call<number>('ai_vector_index_clear'),

  /** 抓取网页（需已开启 AI + 联网档位 + 抓取正文，并带单次确认） */
  webFetch: (url: string, confirmed: boolean) =>
    call<ClippedPage>('ai_web_fetch', { url, confirmed }),

  /** 联网检索（需已开启联网档位 + 本次确认 + 配置好检索源地址） */
  webSearch: (keyword: string, limit = 8, confirmed = false) =>
    call<WebSearchHit[]>('ai_web_search', { keyword, limit, confirmed }),
};

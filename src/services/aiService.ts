import type {
  AiAuditEntry,
  AiPolicy,
  ClippedPage,
  KbHit,
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

  /** 本地检索（复用 FTS5，不联网） */
  kbSearch: (keyword: string, limit = 8) => call<KbHit[]>('ai_kb_search', { keyword, limit }),

  /** 抓取网页（需已开启 AI + 联网档位 + 抓取正文，并带单次确认） */
  webFetch: (url: string, confirmed: boolean) =>
    call<ClippedPage>('ai_web_fetch', { url, confirmed }),

  /** 联网检索（需已开启联网档位 + 本次确认 + 配置好检索源地址） */
  webSearch: (keyword: string, limit = 8, confirmed = false) =>
    call<WebSearchHit[]>('ai_web_search', { keyword, limit, confirmed }),
};

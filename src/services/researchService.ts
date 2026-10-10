import type {
  ResearchRequest,
  ResearchSession,
  ResearchSessionSummary,
  ResearchWriteInput,
  ResearchWriteResult,
} from '@/types';

import { call } from './api';

/**
 * 研究助手服务（V1.2.1 阶段 D1/D2）。
 *
 * 五步固定流程，每步一个命令，UI 逐步推进；闸门与判定都在后端
 * （`application::ai::policy`），这里只转发，避免前后端两套规则。
 */
export const researchService = {
  /** ① 立项：课题 → 研究计划（子问题 + 本地基线） */
  start: (input: ResearchRequest) => call<ResearchSession>('research_start', { input }),

  /** 读取会话（刷新页面后恢复现场） */
  get: (sessionId: string) => call<ResearchSession>('research_get', { sessionId }),

  /** 最近的会话（新的在前） */
  list: (limit = 20) => call<ResearchSessionSummary[]>('research_list', { limit }),

  /** ② 收集：本地 FTS5（外部检索属 D3，`confirmed` 只影响联网路径） */
  collect: (sessionId: string, confirmed = false) =>
    call<ResearchSession>('research_collect', { sessionId, confirmed }),

  /** ③ 验证：≥2 个独立来源才算交叉验证 */
  verify: (sessionId: string) => call<ResearchSession>('research_verify', { sessionId }),

  /** ④ 汇总：渲染固定六段报告 */
  compose: (sessionId: string) => call<ResearchSession>('research_compose', { sessionId }),

  /** ⑤ 写入提案（纯建议，不落库） */
  proposal: (sessionId: string) => call<ResearchSession>('research_proposal', { sessionId }),

  /** 用户确认后落库（默认 Inbox、同标题不覆盖、可回滚） */
  write: (input: ResearchWriteInput) => call<ResearchWriteResult>('research_write', { input }),

  /** 中止（已得材料保留为草稿） */
  abort: (sessionId: string, reason?: string) =>
    call<ResearchSession>('research_abort', { sessionId, reason: reason ?? null }),
};

import { ChevronRight, CircleAlert, Loader2, RefreshCw, Sparkles } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input, Label } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { reportFailure } from '@/lib/actionFeedback';
import { errorMessage, formatDateTime } from '@/lib/format';
import { cn } from '@/lib/utils';
import { aiService } from '@/services/aiService';
import type { AiAuditEntry, AiDiagnosis, AiPolicy, LocalModelPreset } from '@/types';
import type { VectorIndexPreview, VectorIndexStatus } from '@/types';
import type { DecisionProbeReport } from '@/types';
import { EVENTS } from '@/lib/constants';
import { subscribe } from '@/services/api';

/** 自检结论里的档位中文名 */
const TIER_LABEL: Record<AiDiagnosis['tier'], string> = {
  online: '在线模型',
  local: '本地模型',
  template: '模板实现',
};

type AiSettingsTab = 'overview' | 'models' | 'retrieval' | 'advanced';

const AI_SETTINGS_TABS: Array<{ id: AiSettingsTab; label: string }> = [
  { id: 'overview', label: '概览' },
  { id: 'models', label: '模型连接' },
  { id: 'retrieval', label: '联网与增强' },
  { id: 'advanced', label: '决策与高级' },
];

/** 只用于输入框占位，绝不能写进 value 或作为真实 Key 回显。 */
const SAVED_KEY_MASK = '••••••••••••••••••••••••';

/**
 * 「设置 → AI」区块（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.7）。
 *
 * 默认口径：总开关**关闭**、联网档位**仅本地**、抓取正文**关闭**（既定决策，2026-10-10 确认）。
 * 切到「允许联网检索」时必须先看一次说明并确认；判定逻辑全在后端闸门，这里只做展示与提交。
 */
export function AiSettingsPanel() {
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [audit, setAudit] = useState<AiAuditEntry[]>([]);
  /** 审计读取失败的原因：失败不能显示成「还没有 AI 行为记录」 */
  const [auditError, setAuditError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [denylistDraft, setDenylistDraft] = useState('');
  const [allowlistDraft, setAllowlistDraft] = useState('');
  /** API Key 是「只写不读」的：输入框只在本次编辑期间存在，保存后立刻清空 */
  const [remoteKeyDraft, setRemoteKeyDraft] = useState('');
  const [remoteConfirmOpen, setRemoteConfirmOpen] = useState(false);
  /** 触发授权对话框的那个开关：用户确认后要把它一起打开 */
  const [pendingRemoteSwitch, setPendingRemoteSwitch] = useState<'compose' | 'score' | null>(null);
  /** 本地模型预设（来自后端常量）；拿不到就不显示预设区，不阻塞手填 */
  const [presets, setPresets] = useState<LocalModelPreset[]>([]);
  /** 本地模型名的编辑草稿：预设一键填入要能改写输入框，所以这个框改成受控 */
  const [localNameDraft, setLocalNameDraft] = useState('');
  /** 模型自检结论（Track C4）：空 = 还没点过 */
  const [diagnosis, setDiagnosis] = useState<AiDiagnosis | null>(null);
  const [diagnosing, setDiagnosing] = useState(false);
  /** 向量检索（D3）：状态 / 构建确认 / 进度 */
  const [vectorStatus, setVectorStatus] = useState<VectorIndexStatus | null>(null);
  const [vectorPreview, setVectorPreview] = useState<VectorIndexPreview | null>(null);
  const [vectorAskOpen, setVectorAskOpen] = useState(false);
  const [vectorBusy, setVectorBusy] = useState(false);
  const [vectorProgress, setVectorProgress] = useState<{ done: number; total: number } | null>(
    null,
  );
  const [embeddingDraft, setEmbeddingDraft] = useState('');
  /** 决策模型（D4）：配置草稿 + 探针结果 */
  const [decisionBaseDraft, setDecisionBaseDraft] = useState('');
  const [decisionModelDraft, setDecisionModelDraft] = useState('');
  const [decisionKeyDraft, setDecisionKeyDraft] = useState('');
  const [decisionAskOpen, setDecisionAskOpen] = useState(false);
  const [probe, setProbe] = useState<DecisionProbeReport | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [probing, setProbing] = useState(false);
  const [proxyDraft, setProxyDraft] = useState('');
  const [loadError, setLoadError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<AiSettingsTab>('overview');

  /** 读取策略；失败要**说清楚**（原来会一直显示「正在读取…」，看起来像卡住） */
  const load = useCallback(async () => {
    try {
      const next = await aiService.policy();
      setPolicy(next);
      setLoadError(null);
      setDenylistDraft(next.domainDenylist.join('\n'));
      setAllowlistDraft(next.domainAllowlist.join('\n'));
      setLocalNameDraft(next.localModelName ?? '');
      setEmbeddingDraft(next.remoteEmbeddingModel ?? '');
      setDecisionBaseDraft(next.decisionBaseUrl ?? '');
      setDecisionModelDraft(next.decisionModel ?? '');
      setProxyDraft(next.outboundProxy ?? '');
    } catch (error) {
      setLoadError(errorMessage(error));
    }
    try {
      setPresets(await aiService.modelPresets());
    } catch {
      // 预设只是便利项：拿不到就不显示，绝不因此让整个设置页报错
      setPresets([]);
    }
    try {
      setVectorStatus(await aiService.vectorIndexStatus());
    } catch {
      // 状态拿不到不影响其它设置项；构建时会再校验一次
      setVectorStatus(null);
    }
    try {
      setAudit(await aiService.auditTail(5));
      setAuditError(null);
    } catch (error) {
      // 不打断设置页，但**不能**显示成「还没有 AI 行为记录」——那会让用户以为
      // 没人用过 AI，而这里恰恰是用来看「谁在什么时候动过 AI」的地方。
      setAudit([]);
      setAuditError(errorMessage(error));
    }
  }, []);

  useEffect(() => {
    // 放到微任务里再执行：effect 体内同步 setState 会触发级联渲染（仓库里统一的写法）
    void Promise.resolve().then(() => load());
  }, [load]);

  const apply = async (
    input: Parameters<typeof aiService.setPolicy>[0],
    message?: string,
  ): Promise<boolean> => {
    setBusy(true);
    try {
      const next = await aiService.setPolicy(input);
      setPolicy(next);
      if (message) toast.success(message);
      try {
        setAudit(await aiService.auditTail(5));
        setAuditError(null);
      } catch {
        // 审计刷新失败：保留上一次的列表，并记下原因（下面会显示）
        setAuditError('审计刷新失败：下次打开设置页可重试');
      }
      return true;
    } catch (error) {
      reportFailure('保存 AI 设置失败', errorMessage(error));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const parseLines = (value: string) =>
    value
      .split(/[\n,]/)
      .map((item) => item.trim())
      .filter(Boolean);

  /** 自检：只读 + 只在需要时探测本机（不联系在线端点） */
  const runDiagnose = async () => {
    setDiagnosing(true);
    try {
      setDiagnosis(await aiService.diagnose());
    } catch (error) {
      reportFailure('模型自检失败', errorMessage(error));
    } finally {
      setDiagnosing(false);
    }
  };

  /**
   * 向量索引构建（D3）：**先看预览再确认**——这是把整库片段发出去的一次性动作。
   * 构建期间订阅进度事件，别让用户对着"没反应"等几分钟。
   */
  const startVectorBuild = async () => {
    setVectorBusy(true);
    try {
      const preview = await aiService.vectorIndexPreview();
      if (!preview.willSend) {
        toast.info(preview.reason);
        return;
      }
      setVectorPreview(preview);
      setVectorAskOpen(true);
    } catch (error) {
      reportFailure('向量索引预览失败', errorMessage(error));
    } finally {
      setVectorBusy(false);
    }
  };

  const confirmVectorBuild = async () => {
    setVectorAskOpen(false);
    setVectorBusy(true);
    setVectorProgress(null);
    const unlisten = await subscribe<{ done: number; total: number }>(
      EVENTS.vectorIndexProgress,
      (payload) => setVectorProgress(payload),
    ).catch(() => undefined);
    try {
      const report = await aiService.vectorIndexBuild(true);
      toast.success(
        `向量索引已更新：新嵌 ${report.embedded} 条、跳过 ${report.skipped} 条（共 ${report.totalChunks} 条）`,
      );
      setVectorStatus(await aiService.vectorIndexStatus());
    } catch (error) {
      reportFailure('向量索引更新失败', errorMessage(error));
    } finally {
      unlisten?.();
      setVectorProgress(null);
      setVectorBusy(false);
    }
  };

  /** 决策模型契约探针（D4）：用我们自己的中文题看真实指标。 */
  const runDecisionProbe = async () => {
    setProbing(true);
    setProbe(null);
    setProbeError(null);
    try {
      setProbe(await aiService.decisionProbe());
    } catch (error) {
      const message = errorMessage(error);
      setProbeError(message);
      reportFailure('决策探针失败', message);
    } finally {
      setProbing(false);
    }
  };

  if (!policy && loadError) {
    return (
      <div className="space-y-1.5 px-3 py-2 text-xs">
        <p className="text-rose-600 dark:text-rose-300">读取 AI 策略失败：{loadError}</p>
        <Button size="sm" variant="secondary" onClick={() => void load()}>
          重试
        </Button>
      </div>
    );
  }

  if (!policy) {
    return (
      <div className="flex items-center gap-2 px-3 py-2 text-xs text-[var(--ink-subtle)]">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        正在读取 AI 策略…
      </div>
    );
  }

  const remoteConfigured =
    policy.remoteModelKeySet &&
    Boolean(policy.remoteModelBaseUrl?.trim()) &&
    Boolean(policy.remoteModelName?.trim());
  const remoteAuthorized = remoteConfigured && Boolean(policy.remoteModelAuthorizedAt);
  const localConfigured =
    Boolean(policy.localModelEndpoint?.trim()) && Boolean(policy.localModelName?.trim());
  const systemOneConfigured =
    policy.decisionProvider === 'system_one' &&
    Boolean(policy.decisionBaseUrl?.trim()) &&
    Boolean(policy.decisionModel?.trim()) &&
    policy.decisionKeySet &&
    Boolean(policy.decisionAuthorizedAt);
  const systemOneNeedsAttention = policy.decisionProvider === 'system_one' && !systemOneConfigured;
  const scoringTier = systemOneConfigured
    ? 'System One'
    : policy.remoteModelScore && remoteAuthorized
      ? `在线模型 ${policy.remoteModelName}`
      : policy.localModelScore && localConfigured
        ? `本地模型 ${policy.localModelName}`
        : '规则档';
  const composeTier =
    policy.remoteModelCompose && remoteAuthorized
      ? `在线模型 ${policy.remoteModelName}`
      : policy.localModelCompose && localConfigured
        ? `本地模型 ${policy.localModelName}`
        : '模板档';
  const networkStatus =
    policy.network === 'localOnly'
      ? '仅本地'
      : policy.searchEndpoint
        ? '联网源已配置'
        : '待配置检索源';
  const openTab = (tab: AiSettingsTab, targetId?: string) => {
    setActiveTab(tab);
    if (!targetId) return;
    window.requestAnimationFrame(() => {
      const target = document.getElementById(targetId);
      target?.scrollIntoView?.({ block: 'center' });
      if (target instanceof HTMLElement) target.focus({ preventScroll: true });
    });
  };
  const nextSteps: Array<{ text: string; tab: AiSettingsTab; targetId?: string }> = [];
  if (!policy.enabled) {
    nextSteps.push({ text: '打开 AI 能力总开关', tab: 'overview', targetId: 'ai-enabled' });
  }
  if ((policy.remoteModelCompose || policy.remoteModelScore) && !remoteConfigured) {
    nextSteps.push({
      text: '补齐在线模型地址、模型名与 API Key',
      tab: 'models',
      targetId: 'ai-remote-endpoint',
    });
  } else if (
    remoteConfigured &&
    !policy.remoteModelAuthorizedAt &&
    (policy.remoteModelCompose || policy.remoteModelScore)
  ) {
    nextSteps.push({
      text: '确认在线模型外发授权',
      tab: 'models',
      targetId: 'ai-remote-authorize',
    });
  }
  if (remoteAuthorized && !policy.remoteModelScore) {
    nextSteps.push({
      text: '如需模型判断来源，打开“用在线模型打分”',
      tab: 'models',
      targetId: 'ai-remote-score',
    });
  }
  if (policy.network === 'allowNetwork' && !policy.searchEndpoint) {
    nextSteps.push({
      text: '如需外部网页来源，配置检索源地址',
      tab: 'retrieval',
      targetId: 'ai-search-endpoint',
    });
  }
  if (systemOneNeedsAttention) {
    nextSteps.push({
      text: '完成 System One 配置与授权，或切回规则档',
      tab: 'advanced',
      targetId: 'ai-decision-provider',
    });
  }
  if (policy.vectorEnabled && (!policy.remoteEmbeddingModel || !policy.vectorAuthorizedAt)) {
    nextSteps.push({
      text: '补齐嵌入模型并授权向量检索',
      tab: 'retrieval',
      targetId: 'ai-embedding-model',
    });
  }

  return (
    <div className="divide-y divide-[var(--border-subtle)]">
      <section
        data-testid="ai-status-overview"
        className="space-y-3 bg-[var(--surface-muted)]/45 px-3 py-3"
      >
        <div className="flex flex-wrap items-start justify-between gap-2">
          <div>
            <h3 className="text-sm font-medium text-[var(--ink)]">当前生效链路</h3>
            <p className="mt-0.5 text-xs leading-relaxed text-[var(--ink-subtle)]">
              “已配置”不等于“会调用”；总开关、配置、授权和用途开关分别生效。
            </p>
          </div>
          <StatusPill
            tone={policy.enabled ? 'ok' : 'muted'}
            label={policy.enabled ? 'AI 已开启' : 'AI 已关闭'}
          />
        </div>
        <div className="grid gap-2 sm:grid-cols-2">
          <StatusCard
            label="联网检索"
            value={networkStatus}
            tone={policy.network === 'allowNetwork' && policy.searchEndpoint ? 'ok' : 'muted'}
            onClick={() => openTab('retrieval', 'ai-search-endpoint')}
          />
          <StatusCard
            label="来源打分"
            value={scoringTier}
            tone={scoringTier === '规则档' ? 'muted' : 'ok'}
            onClick={() => openTab('models', 'ai-remote-score')}
          />
          <StatusCard
            label="报告汇总"
            value={composeTier}
            tone={composeTier === '模板档' ? 'muted' : 'ok'}
            onClick={() => openTab('models', 'ai-remote-compose')}
          />
          <StatusCard
            label="向量检索"
            value={
              policy.vectorEnabled ? (policy.vectorAuthorizedAt ? '已启用' : '待授权') : '关闭'
            }
            tone={policy.vectorEnabled && policy.vectorAuthorizedAt ? 'ok' : 'muted'}
            onClick={() => openTab('retrieval', 'ai-embedding-model')}
          />
        </div>
        {nextSteps.length > 0 ? (
          <div className="rounded-lg border border-amber-500/35 bg-amber-500/10 px-2 py-1.5 text-xs text-amber-900 dark:text-amber-200">
            <p className="font-medium">还需要你确认</p>
            <ul className="mt-1 space-y-0.5">
              {nextSteps.map((step) => (
                <li key={step.text}>
                  <button
                    type="button"
                    aria-label={`处理：${step.text}`}
                    className="group flex min-h-9 w-full items-center justify-between gap-3 rounded-md px-2 py-1 text-left transition-colors hover:bg-amber-500/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500"
                    onClick={() => openTab(step.tab, step.targetId)}
                  >
                    <span>{step.text}</span>
                    <span className="flex shrink-0 items-center gap-0.5 underline underline-offset-2">
                      去处理
                      <ChevronRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <p className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-2 text-xs text-emerald-700 dark:text-emerald-300">
            当前已开启的能力都满足生效条件。
          </p>
        )}
      </section>

      <div className="sticky top-0 z-20 border-y border-[var(--border-subtle)] bg-[var(--surface)] px-3 py-2 shadow-sm">
        <nav role="tablist" aria-label="AI 设置分区" className="flex flex-wrap gap-1.5">
          {AI_SETTINGS_TABS.map(({ id, label }, index) => (
            <button
              key={id}
              id={`ai-tab-${id}`}
              type="button"
              role="tab"
              aria-selected={activeTab === id}
              aria-controls={`ai-tabpanel-${id}`}
              tabIndex={activeTab === id ? 0 : -1}
              data-testid={`ai-tab-${id}`}
              onClick={() => setActiveTab(id)}
              onKeyDown={(event) => {
                const nextIndex =
                  event.key === 'ArrowRight'
                    ? (index + 1) % AI_SETTINGS_TABS.length
                    : event.key === 'ArrowLeft'
                      ? (index - 1 + AI_SETTINGS_TABS.length) % AI_SETTINGS_TABS.length
                      : event.key === 'Home'
                        ? 0
                        : event.key === 'End'
                          ? AI_SETTINGS_TABS.length - 1
                          : null;
                if (nextIndex === null) return;
                const next = AI_SETTINGS_TABS[nextIndex];
                if (!next) return;
                event.preventDefault();
                setActiveTab(next.id);
                window.requestAnimationFrame(() =>
                  document.getElementById(`ai-tab-${next.id}`)?.focus(),
                );
              }}
              className={cn(
                'min-h-9 rounded-full border px-3 py-1.5 text-xs transition-[color,background-color,border-color,box-shadow] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]',
                activeTab === id
                  ? 'border-[var(--brand)] bg-[var(--brand-soft)] font-medium text-[var(--brand)] shadow-sm'
                  : 'border-[var(--border-subtle)] bg-[var(--surface)] text-[var(--ink-muted)] hover:border-[var(--border-strong)] hover:text-[var(--ink)]',
              )}
            >
              {label}
            </button>
          ))}
        </nav>
      </div>

      <div
        id="ai-tabpanel-overview"
        role="tabpanel"
        aria-labelledby="ai-tab-overview"
        data-testid="ai-tabpanel-overview"
        className={activeTab === 'overview' ? 'contents' : 'hidden'}
      >
        <SectionHeading
          id="ai-core"
          title="核心能力"
          description="先打开总开关，再按需要启用联网、模型或决策能力。"
        />
        <Row label="AI 能力总开关" hint="默认关闭；关闭时所有 AI 命令都会被后端拒绝">
          <Switch
            id="ai-enabled"
            data-testid="ai-enabled"
            checked={policy.enabled}
            onCheckedChange={(checked) =>
              void apply(
                { enabled: checked },
                checked ? '已开启 AI 能力（仍不会自动联网）' : '已关闭 AI 能力',
              )
            }
          />
        </Row>

        <Row
          label="模型自检"
          hint="一键看「写报告」现在用哪一档，以及为什么（只读，不联系在线端点）"
        >
          <Button
            size="sm"
            variant="secondary"
            data-testid="ai-diagnose"
            disabled={diagnosing}
            onClick={() => void runDiagnose()}
          >
            {diagnosing ? '检测中…' : '开始自检'}
          </Button>
        </Row>
        {diagnosis ? (
          <div className="space-y-1 px-3 pb-2 text-xs" data-testid="ai-diagnosis">
            <p className="text-[var(--ink)]">
              当前档位：
              <span data-testid="ai-diagnosis-tier" className="font-medium">
                {TIER_LABEL[diagnosis.tier]}
              </span>
              <span className="text-[var(--ink-subtle)]">（{diagnosis.providerId}）</span>
            </p>
            <p data-testid="ai-diagnosis-reason" className="text-[var(--ink-subtle)]">
              {diagnosis.reason}
            </p>
            <ul className="space-y-0.5">
              {diagnosis.checks.map((check) => (
                <li key={check.tier} className="text-[var(--ink-subtle)]">
                  {check.available ? '✓' : '—'} {check.label}：{check.detail}
                </li>
              ))}
            </ul>
            {diagnosis.note ? <p className="text-[var(--ink-subtle)]">{diagnosis.note}</p> : null}
          </div>
        ) : null}
      </div>

      <div
        id="ai-tabpanel-retrieval"
        role="tabpanel"
        aria-labelledby="ai-tab-retrieval"
        data-testid="ai-tabpanel-retrieval"
        className={activeTab === 'retrieval' ? 'contents' : 'hidden'}
      >
        <SectionHeading
          id="ai-network-section"
          title="联网检索与来源"
          description="联网检索、在线模型、向量索引是三个独立权限；任一开启都不代表另外两个开启。"
        />
        <Row label="联网检索" hint="允许联网后，每次研究收集仍需单次确认">
          <select
            aria-label="联网档位"
            data-testid="ai-network"
            value={policy.network}
            onChange={(event) => {
              const next = event.target.value as AiPolicy['network'];
              if (next === 'allowNetwork') {
                setConfirmOpen(true);
                return;
              }
              void apply({ network: 'localOnly' }, '已切回「仅本地」');
            }}
            className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
          >
            <option value="localOnly">仅本地</option>
            <option value="allowNetwork">允许联网检索</option>
          </select>
        </Row>

        <Row label="抓取网页正文" hint="默认关闭（可只用搜索摘要）；开启后按域名策略抓公开页面">
          <Switch
            data-testid="ai-fetch-body"
            checked={policy.allowFetchBody}
            onCheckedChange={(checked) => void apply({ allowFetchBody: checked })}
          />
        </Row>

        <SectionHeading
          id="ai-search-policy"
          title="检索源与域名策略"
          description="应用不预置第三方搜索服务；白名单为空表示按内置来源分级规则处理。"
        />
        <div className="space-y-1.5 px-3 py-2">
          <div className="space-y-1">
            <Label htmlFor="ai-search-endpoint">检索源地址（联网检索用，必须含 {'{query}'}）</Label>
            <Input
              id="ai-search-endpoint"
              data-testid="ai-search-endpoint"
              defaultValue={policy.searchEndpoint ?? ''}
              placeholder="https://your-searx.example/search?q={query}&format=json&count={limit}"
              onBlur={(event) => void apply({ searchEndpoint: event.target.value }, '已保存检索源')}
            />
            <p className="text-[11px] text-[var(--ink-subtle)]">
              应用不预置任何第三方检索服务：这里填你自己可用的检索接口（返回 JSON， 支持 SearxNG 的{' '}
              {'{ "results": [ … ] }'} 形状）。留空表示不联网检索； 地址不合法或缺少 {'{query}'}{' '}
              会被清空。
            </p>
          </div>
          <div className="space-y-1">
            <Label htmlFor="ai-allowlist">域名白名单（一行一个，留空 = 不预置白名单）</Label>
            <textarea
              id="ai-allowlist"
              data-testid="ai-allowlist"
              value={allowlistDraft}
              rows={2}
              placeholder={'docs.example.com\nblog.example.com'}
              onChange={(event) => setAllowlistDraft(event.target.value)}
              onBlur={() =>
                void apply({ domainAllowlist: parseLines(allowlistDraft) }, '已保存白名单')
              }
              className="w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 py-1.5 text-xs text-[var(--ink)]"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="ai-denylist">域名黑名单（强制丢弃，优先级高于白名单）</Label>
            <textarea
              id="ai-denylist"
              data-testid="ai-denylist"
              value={denylistDraft}
              rows={2}
              placeholder={'ads.example.com'}
              onChange={(event) => setDenylistDraft(event.target.value)}
              onBlur={() =>
                void apply({ domainDenylist: parseLines(denylistDraft) }, '已保存黑名单')
              }
              className="w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 py-1.5 text-xs text-[var(--ink)]"
            />
          </div>
        </div>

        <SectionHeading
          id="ai-vector"
          title="向量检索"
          description="可选的召回增强；关闭或不可用时自动回退关键词检索。"
        />
        {/* 向量检索（D3/N7）：可选增强，永远只是"多一路召回" —— 不可用也不影响检索 */}
        <div className="space-y-1.5 px-3 py-2">
          <div className="flex items-center justify-between">
            <Label>向量检索（可选增强，默认关闭）</Label>
            <Switch
              data-testid="ai-vector-enabled"
              checked={policy.vectorEnabled}
              onCheckedChange={(checked) => {
                if (checked && !policy.vectorAuthorizedAt) {
                  // 开启=查询词会发给嵌入端点：先让它填模型名再谈授权
                  void apply(
                    { vectorEnabled: true },
                    '已开启向量检索（还需填嵌入模型名并确认一次）',
                  );
                  return;
                }
                void apply({ vectorEnabled: checked });
              }}
            />
          </div>
          <Input
            id="ai-embedding-model"
            aria-label="嵌入模型名"
            data-testid="ai-embedding-model"
            value={embeddingDraft}
            onChange={(event) => setEmbeddingDraft(event.target.value)}
            onBlur={() => {
              if (embeddingDraft.trim() !== (policy.remoteEmbeddingModel ?? '')) {
                void apply({ remoteEmbeddingModel: embeddingDraft }, '已保存嵌入模型名');
              }
            }}
            placeholder="例如 text-embedding-3-small（嵌入模型名，与聊天模型不同）"
          />
          <p data-testid="ai-vector-status" className="text-[11px] text-[var(--ink-subtle)]">
            {vectorStatus?.note ?? '正在读取向量索引状态…'}
            {policy.vectorAuthorizedAt
              ? `；已授权（${formatDateTime(policy.vectorAuthorizedAt)}）`
              : '；尚未授权（查询词会发给嵌入端点，需确认一次）'}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant="secondary"
              data-testid="ai-vector-authorize"
              disabled={!policy.vectorEnabled || policy.vectorAuthorizedAt !== null}
              onClick={() =>
                void apply(
                  { confirmVector: true },
                  '已授权：查询词会发给嵌入端点（换地址/换模型后需重新确认）',
                )
              }
            >
              授权向量检索
            </Button>
            <Button
              size="sm"
              variant="secondary"
              data-testid="ai-vector-build"
              disabled={vectorBusy || !policy.vectorEnabled}
              onClick={() => void startVectorBuild()}
            >
              {vectorBusy ? '处理中…' : '构建向量索引'}
            </Button>
            {vectorProgress ? (
              <span
                data-testid="ai-vector-progress"
                className="text-[11px] text-[var(--ink-subtle)]"
              >
                已嵌 {vectorProgress.done} / {vectorProgress.total}
              </span>
            ) : null}
          </div>
          <p className="text-[11px] text-[var(--ink-subtle)]">
            只能增强召回，「不能」决定检索是否可用：向量档不可用时自动只用关键词检索。
            构建是增量且需确认的一次性动作（只有内容变过的片段才会重发）。
          </p>
        </div>
      </div>

      <div
        id="ai-tabpanel-models"
        role="tabpanel"
        aria-labelledby="ai-tab-models"
        data-testid="ai-tabpanel-models"
        className={activeTab === 'models' ? 'contents' : 'hidden'}
      >
        <SectionHeading
          id="ai-local-section"
          title="本地模型"
          description="只连接本机 Ollama；打分与写报告是两个独立用途。"
        />
        <div className="space-y-1.5 px-3 py-2">
          <div className="space-y-1">
            <Label htmlFor="ai-local-endpoint">Ollama 地址</Label>
            <Input
              id="ai-local-endpoint"
              aria-label="本地模型地址"
              data-testid="ai-local-endpoint"
              defaultValue={policy.localModelEndpoint ?? ''}
              placeholder="http://127.0.0.1:11434"
              onBlur={(event) =>
                void apply({ localModelEndpoint: event.target.value }, '已保存本地模型地址')
              }
            />
            <Label htmlFor="ai-local-model-name">模型名</Label>
            <Input
              id="ai-local-model-name"
              aria-label="本地模型名称"
              data-testid="ai-local-model"
              value={localNameDraft}
              onChange={(event) => setLocalNameDraft(event.target.value)}
              onBlur={() => {
                if (localNameDraft.trim() !== (policy.localModelName ?? '')) {
                  void apply({ localModelName: localNameDraft }, '已保存模型名');
                }
              }}
              placeholder="qwen2.5:7b（模型名，需先 ollama pull）"
            />
          </div>
          {presets.length > 0 ? (
            <div className="space-y-1" data-testid="ai-local-presets">
              <p className="text-[11px] text-[var(--ink-subtle)]">
                不确定选哪个？用预设一键填入。体积为「近似」下载体积，以 Ollama 实际下载为准：
              </p>
              {presets.map((preset) => (
                <div key={preset.name} className="flex items-start justify-between gap-2">
                  <div className="space-y-0.5">
                    <p className="text-[11px] text-[var(--ink)]">
                      {preset.label} · {preset.approxSize}
                    </p>
                    <p className="text-[11px] text-[var(--ink-subtle)]">
                      {preset.memoryHint}；{preset.note}
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="secondary"
                    data-testid={`ai-local-preset-${preset.name}`}
                    disabled={busy || policy.localModelName === preset.name}
                    onClick={() => {
                      setLocalNameDraft(preset.name);
                      void apply({ localModelName: preset.name }, `已填入 ${preset.name}`);
                    }}
                  >
                    {policy.localModelName === preset.name ? '已选用' : `填入 ${preset.name}`}
                  </Button>
                </div>
              ))}
            </div>
          ) : null}
          <Row label="用本地模型写报告" hint="只影响「模型推测」段的文字；失败自动退回模板实现">
            <Switch
              data-testid="ai-local-compose"
              checked={policy.localModelCompose}
              onCheckedChange={(checked) => void apply({ localModelCompose: checked })}
            />
          </Row>
          <Row
            label="用本地模型打分"
            hint="与规则档 A/B 用；模型分未校准，级别折算仍占一半（不是 Jev）"
          >
            <Switch
              data-testid="ai-local-score"
              checked={policy.localModelScore}
              onCheckedChange={(checked) => void apply({ localModelScore: checked })}
            />
          </Row>
          <p className="text-[11px] text-[var(--ink-subtle)]">
            只连本机地址（localhost / 127.0.0.1 / ::1）；两项开关都关闭时一个请求都不会发；
            浏览器预览模式不会真的调用本地模型。装不上模型时用下面的「在线模型」。
          </p>
        </div>

        {/* 在线模型：给「跑不动本地模型」的机器留的一档（本地软件、跨平台部署是常态） */}
        <SectionHeading
          id="ai-online-model"
          title="在线模型"
          description="地址、模型名、Key 和授权是生效前提；“打分”与“写报告”按需分别开启。"
        />
        <div className="space-y-1.5 px-3 py-2">
          <div className="flex items-center justify-between">
            <Label>OpenAI 兼容连接</Label>
            <KeyStatusBadge configured={policy.remoteModelKeySet} testId="ai-remote-key-badge" />
          </div>
          <div className="space-y-1">
            <Label htmlFor="ai-remote-endpoint">模型基地址</Label>
            <Input
              id="ai-remote-endpoint"
              aria-label="在线模型地址"
              data-testid="ai-remote-endpoint"
              defaultValue={policy.remoteModelBaseUrl ?? ''}
              placeholder="https://api.example.com/v1（必须是 https 的公网域名）"
              onBlur={(event) =>
                void apply({ remoteModelBaseUrl: event.target.value }, '已保存在线模型地址')
              }
            />
            <Label htmlFor="ai-remote-model">模型名</Label>
            <Input
              id="ai-remote-model"
              aria-label="在线模型名称"
              data-testid="ai-remote-model"
              defaultValue={policy.remoteModelName ?? ''}
              placeholder="deepseek-chat（服务商提供的模型名）"
              onBlur={(event) =>
                void apply({ remoteModelName: event.target.value }, '已保存模型名')
              }
            />
            <Label htmlFor="ai-remote-key">API Key（只写不读）</Label>
            <div className="flex items-center gap-2">
              <Input
                id="ai-remote-key"
                aria-label="在线模型 API Key"
                data-testid="ai-remote-key"
                type="password"
                autoComplete="off"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                value={remoteKeyDraft}
                placeholder={
                  policy.remoteModelKeySet
                    ? `${SAVED_KEY_MASK} 已配置，输入新 Key 可覆盖`
                    : 'API Key（只写不读，保存后无法再查看）'
                }
                onChange={(event) => setRemoteKeyDraft(event.target.value)}
              />
              <Button
                size="sm"
                variant="secondary"
                data-testid="ai-remote-key-save"
                disabled={busy || remoteKeyDraft.trim().length === 0}
                onClick={() => {
                  const key = remoteKeyDraft;
                  void apply(
                    { remoteModelApiKey: key },
                    '已保存 API Key（存储在本机应用数据目录）',
                  ).then((saved) => {
                    // 只写不读：成功落盘后才清空，失败时保留用户唯一副本。
                    if (saved) setRemoteKeyDraft('');
                  });
                }}
              >
                保存
              </Button>
              <Button
                size="sm"
                variant="ghost"
                data-testid="ai-remote-key-clear"
                disabled={busy || !policy.remoteModelKeySet}
                onClick={() => {
                  setRemoteKeyDraft('');
                  void apply({ remoteModelApiKey: '' }, '已清除 API Key');
                }}
              >
                清除
              </Button>
            </div>
          </div>
          <Row label="用在线模型写报告" hint="材料片段会离开本机；失败自动降级到本地模型或模板实现">
            <Switch
              id="ai-remote-compose"
              data-testid="ai-remote-compose"
              checked={policy.remoteModelCompose}
              onCheckedChange={(checked) => {
                if (checked && !policy.remoteModelAuthorizedAt) {
                  setPendingRemoteSwitch('compose');
                  setRemoteConfirmOpen(true);
                  return;
                }
                void apply({ remoteModelCompose: checked });
              }}
            />
          </Row>
          <Row label="用在线模型打分" hint="与规则档 A/B 用；同样只在授权后外发">
            <Switch
              id="ai-remote-score"
              data-testid="ai-remote-score"
              checked={policy.remoteModelScore}
              onCheckedChange={(checked) => {
                if (checked && !policy.remoteModelAuthorizedAt) {
                  setPendingRemoteSwitch('score');
                  setRemoteConfirmOpen(true);
                  return;
                }
                void apply({ remoteModelScore: checked });
              }}
            />
          </Row>
          <Row
            label="每次外发前确认"
            hint="开启后：每一次把内容交给在线模型都要单独确认（未确认时一个字节都不发）"
          >
            <Switch
              data-testid="ai-confirm-each-outbound"
              checked={policy.confirmEachOutbound}
              onCheckedChange={(checked) =>
                void apply(
                  { confirmEachOutbound: checked },
                  checked ? '已开启逐次外发确认' : '已关闭逐次外发确认',
                )
              }
            />
          </Row>
          <p data-testid="ai-remote-authorized" className="text-[11px] text-[var(--ink-subtle)]">
            {policy.remoteModelAuthorizedAt
              ? `已授权外发（${formatDateTime(policy.remoteModelAuthorizedAt)}）；换地址或模型名后需要重新确认。`
              : remoteConfigured
                ? '地址、模型名与 Key 已填写，但尚未授权；当前不会向该地址发送内容。'
                : '尚未授权：这一档不会向任何地址发送内容。'}
          </p>
          {remoteConfigured && !policy.remoteModelAuthorizedAt ? (
            <Button
              id="ai-remote-authorize"
              size="sm"
              variant="secondary"
              data-testid="ai-remote-authorize"
              onClick={() => {
                setPendingRemoteSwitch(null);
                setRemoteConfirmOpen(true);
              }}
            >
              授权在线模型外发
            </Button>
          ) : null}
          <p className="text-[11px] text-[var(--ink-subtle)]">
            给「本机跑不动本地模型」的机器留的一档：地址只接受 https 的公网域名（本机可写
            localhost），裸 IP 与非加密地址会被拒绝。Key 存在本机应用数据目录（不进工作区、
            不进备份）；本节的外发由独立授权控制，与上面的「联网检索」档位互不影响。
          </p>
        </div>
      </div>

      <div
        id="ai-tabpanel-advanced"
        role="tabpanel"
        aria-labelledby="ai-tab-advanced"
        data-testid="ai-tabpanel-advanced"
        className={activeTab === 'advanced' ? 'contents' : 'hidden'}
      >
        {/* 决策模型（D4）：规则档（默认，离线）/ System One（Jev 或 Perplexity Decisions，同协议） */}
        <SectionHeading
          id="ai-decision-model"
          title="决策模型"
          description="用于来源相关性、冲突与校准判断；规则档始终离线可用。"
        />
        <div className="space-y-1.5 px-3 py-2">
          <Label htmlFor="ai-decision-provider">打分 / 判断档位</Label>
          <select
            id="ai-decision-provider"
            aria-label="决策模型档位"
            data-testid="ai-decision-provider"
            value={policy.decisionProvider ?? 'rule'}
            onChange={(event) => {
              const next = event.target.value as 'rule' | 'system_one';
              void apply(
                { decisionProvider: next },
                next === 'rule'
                  ? '已切回规则档'
                  : '已选择 System One（还需填端点、模型与 Key 并授权）',
              );
            }}
            className="h-8 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
          >
            <option value="rule">规则档（默认，离线、不联网）</option>
            <option value="system_one">System One（TypeSafe Jev / Perplexity Decisions）</option>
          </select>
          {policy.decisionProvider === 'system_one' ? (
            <>
              <div className="flex flex-wrap gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  data-testid="ai-decision-preset-perplexity"
                  onClick={() =>
                    void apply(
                      {
                        decisionBaseUrl: 'https://api.perplexity.ai',
                        decisionModel: 'pplx-decider-v1.1-27b',
                      },
                      '已填入 Perplexity 预设（还需填 Key 并授权）',
                    )
                  }
                >
                  用 Perplexity 预设
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  data-testid="ai-decision-preset-jev"
                  onClick={() =>
                    void apply(
                      { decisionBaseUrl: 'https://api.typesafe.ai', decisionModel: 'jev-latest' },
                      '已填入 Jev 预设（还需填 Key 并授权）',
                    )
                  }
                >
                  用 Jev 预设
                </Button>
              </div>
              <div className="space-y-1">
                <Label htmlFor="ai-decision-base">模型基地址</Label>
                <Input
                  id="ai-decision-base"
                  aria-label="决策模型基地址"
                  data-testid="ai-decision-base"
                  value={decisionBaseDraft}
                  onChange={(event) => setDecisionBaseDraft(event.target.value)}
                  onBlur={() => {
                    if (decisionBaseDraft.trim() !== (policy.decisionBaseUrl ?? '')) {
                      void apply({ decisionBaseUrl: decisionBaseDraft }, '已保存决策模型地址');
                    }
                  }}
                  placeholder="https://api.perplexity.ai（只接受 https 公网域名）"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="ai-decision-model">模型名</Label>
                <Input
                  id="ai-decision-model"
                  aria-label="决策模型名"
                  data-testid="ai-decision-model"
                  value={decisionModelDraft}
                  onChange={(event) => setDecisionModelDraft(event.target.value)}
                  onBlur={() => {
                    if (decisionModelDraft.trim() !== (policy.decisionModel ?? '')) {
                      void apply({ decisionModel: decisionModelDraft }, '已保存决策模型名');
                    }
                  }}
                  placeholder="pplx-decider-v1.1-27b / jev-latest"
                />
              </div>
              <div className="flex items-center justify-between gap-2">
                <Label htmlFor="ai-decision-key">API Key（只写不读）</Label>
                <KeyStatusBadge configured={policy.decisionKeySet} testId="ai-decision-key-badge" />
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Input
                  id="ai-decision-key"
                  aria-label="决策模型 API Key"
                  data-testid="ai-decision-key"
                  type="password"
                  autoComplete="off"
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  value={decisionKeyDraft}
                  onChange={(event) => setDecisionKeyDraft(event.target.value)}
                  placeholder={
                    policy.decisionKeySet
                      ? `${SAVED_KEY_MASK} 已配置，输入新 Key 可覆盖`
                      : 'API Key（只写不读，保存后无法再查看）'
                  }
                />
                <Button
                  size="sm"
                  variant="secondary"
                  data-testid="ai-decision-key-save"
                  disabled={busy || decisionKeyDraft.trim().length === 0}
                  onClick={() => {
                    void apply({ decisionApiKey: decisionKeyDraft }, '已保存决策模型 Key').then(
                      (saved) => {
                        // 与在线 Key 同一纪律：保存失败不能把输入框里的唯一副本清掉。
                        if (saved) setDecisionKeyDraft('');
                      },
                    );
                  }}
                >
                  保存 Key
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  data-testid="ai-decision-key-clear"
                  disabled={busy || !policy.decisionKeySet}
                  onClick={() => void apply({ decisionApiKey: '' }, '已清除决策模型 Key')}
                >
                  清除
                </Button>
              </div>
              <p
                data-testid="ai-decision-authorized"
                className="text-[11px] text-[var(--ink-subtle)]"
              >
                {policy.decisionAuthorizedAt
                  ? `已授权外发（${formatDateTime(policy.decisionAuthorizedAt)}）；换地址或模型名后需要重新确认。`
                  : '尚未授权：这一档不会向任何地址发送内容。'}
              </p>
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  data-testid="ai-decision-authorize"
                  disabled={policy.decisionAuthorizedAt !== null}
                  onClick={() => setDecisionAskOpen(true)}
                >
                  授权决策模型
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  data-testid="ai-decision-probe"
                  disabled={probing || policy.decisionAuthorizedAt === null}
                  onClick={() => void runDecisionProbe()}
                >
                  {probing ? '探针运行中…' : '运行契约探针'}
                </Button>
              </div>
              {probeError ? (
                <div
                  role="alert"
                  data-testid="ai-decision-probe-error"
                  className="space-y-1 rounded-lg border border-red-500/35 bg-red-500/10 p-2.5 text-xs text-red-700 dark:text-red-200"
                >
                  <p className="flex items-center gap-1.5 font-medium">
                    <CircleAlert className="h-4 w-4 shrink-0" />
                    探针未通过
                  </p>
                  <p className="break-words leading-relaxed">{probeError}</p>
                  {probeError.includes('HTTP 401') ? (
                    <p className="leading-relaxed">
                      401 表示服务端没有接受当前 Key。请到 Perplexity API 控制台创建或轮换专用 API
                      Key，确认复制的是完整明文而不是掩码/占位符，然后先点“清除”再保存新 Key
                      并重跑探针。DeepSeek 的 Key 不能用于 Perplexity 决策端点。
                    </p>
                  ) : null}
                </div>
              ) : null}
              {probe ? (
                <div
                  data-testid="ai-decision-probe-report"
                  className="space-y-1 rounded-lg border border-[var(--border-subtle)] p-2 text-[11px] text-[var(--ink-muted)]"
                >
                  <p>
                    模型 {probe.model}（{probe.host}）：准确率{' '}
                    {probe.accuracy === null || probe.accuracy === undefined
                      ? '—'
                      : `${Math.round(probe.accuracy * 100)}%（${probe.scoredItems} 题）`}
                    ，自洽性 {probe.coherence?.toFixed(3) ?? '—'}，两次一致{' '}
                    {probe.deterministic ? '是' : '否'}，耗时 {probe.latencyMs} ms，输入{' '}
                    {probe.inputTokens} token（≈${probe.estimatedUsd.toFixed(6)}）
                  </p>
                  <p>
                    矛盾判别：真矛盾 {probe.conflictPairYes?.toFixed(3) ?? '—'} / 不矛盾{' '}
                    {probe.nonConflictPairYes?.toFixed(3) ?? '—'}
                  </p>
                  <p data-testid="ai-decision-calibration">
                    ECE {probe.calibration.ece.toFixed(4)}（线 0.05）· Brier{' '}
                    {probe.calibration.brier.toFixed(4)} · 折外{' '}
                    {probe.outOfSampleEce === null || probe.outOfSampleEce === undefined
                      ? '—'
                      : probe.outOfSampleEce.toFixed(4)}
                  </p>
                  <p data-testid="ai-decision-calibration-families">
                    确定性题 ECE {probe.calibrationDeterminable.ece.toFixed(4)}（
                    {probe.calibrationDeterminable.samples} 题）· 材料外题 ECE{' '}
                    {probe.calibrationUnderdetermined.ece.toFixed(4)}（
                    {probe.calibrationUnderdetermined.samples} 题），平均说{' '}
                    {probe.meanStatedUnderdetermined === null ||
                    probe.meanStatedUnderdetermined === undefined
                      ? '—'
                      : probe.meanStatedUnderdetermined.toFixed(3)}
                    （理想 ≈0.5）
                  </p>
                  {probe.calibration.bins.length > 0 ? (
                    <ul className="space-y-0.5">
                      {probe.calibration.bins.map((bin) => (
                        <li key={bin.range}>
                          {bin.range}：说 {bin.meanStated.toFixed(3)} → 实际{' '}
                          {bin.actualShare.toFixed(3)}（{bin.count} 条）
                        </li>
                      ))}
                    </ul>
                  ) : null}
                  <p data-testid="ai-decision-ab">
                    A/B（{probe.ab.cases} 条打分，相关/不相关各半）：规则档 {probe.ab.rule.correct}/
                    {probe.ab.rule.total}（区分度 {probe.ab.rule.separation.toFixed(3)}）· 决策档{' '}
                    {probe.ab.decision.correct}/{probe.ab.decision.total}（区分度{' '}
                    {probe.ab.decision.separation.toFixed(3)}）→{' '}
                    {probe.ab.decisionBetter ? '决策档更好' : '规则档不落下风'}
                  </p>
                  <p>{probe.note}</p>
                </div>
              ) : null}
            </>
          ) : null}
        </div>

        {/* 出站代理（D4-④）：默认跟随系统代理；"只有浏览器能上外网"时在这里兜底 */}
        <SectionHeading
          id="ai-advanced"
          title="连接与预算"
          description="代理和研究预算只影响外部请求的组织方式。"
        />
        <div className="space-y-1.5 px-3 py-2">
          <Label htmlFor="ai-outbound-proxy">出站代理（留空 = 跟随系统代理）</Label>
          <Input
            id="ai-outbound-proxy"
            aria-label="出站代理"
            data-testid="ai-outbound-proxy"
            value={proxyDraft}
            onChange={(event) => setProxyDraft(event.target.value)}
            onBlur={() => {
              if (proxyDraft.trim() !== (policy.outboundProxy ?? '')) {
                void apply({ outboundProxy: proxyDraft }, '已保存出站代理');
              }
            }}
            placeholder="http://127.0.0.1:8118 或 socks5h://127.0.0.1:8119"
          />
          <p className="text-[11px] text-[var(--ink-subtle)]">
            按「显式设置 → 环境变量 → macOS 系统代理」的顺序生效；本机地址永远不会走代理
            （本地模型不受影响）。
          </p>
        </div>

        <div id="ai-audit" className="scroll-mt-4 space-y-1.5 px-3 py-2">
          <Label>研究预算（超出即停并汇报进度）</Label>
          <div className="flex flex-wrap items-center gap-3 text-xs text-[var(--ink-muted)]">
            <label className="flex items-center gap-1.5">
              子问题
              <Input
                data-testid="ai-budget-subquestions"
                type="number"
                min={1}
                max={10}
                value={policy.budget.maxSubquestions}
                onChange={(event) =>
                  void apply({
                    budget: { ...policy.budget, maxSubquestions: Number(event.target.value) },
                  })
                }
                className="h-7 w-16 text-xs"
              />
            </label>
            <label className="flex items-center gap-1.5">
              抓取页
              <Input
                data-testid="ai-budget-pages"
                type="number"
                min={1}
                max={50}
                value={policy.budget.maxPages}
                onChange={(event) =>
                  void apply({ budget: { ...policy.budget, maxPages: Number(event.target.value) } })
                }
                className="h-7 w-16 text-xs"
              />
            </label>
            <label className="flex items-center gap-1.5">
              时长（秒）
              <Input
                data-testid="ai-budget-seconds"
                type="number"
                min={30}
                max={1800}
                value={policy.budget.maxDurationSecs}
                onChange={(event) =>
                  void apply({
                    budget: { ...policy.budget, maxDurationSecs: Number(event.target.value) },
                  })
                }
                className="h-7 w-20 text-xs"
              />
            </label>
          </div>
        </div>

        <div className="space-y-1.5 px-3 py-2">
          <div className="flex items-center justify-between">
            <Label>最近行为（审计不含正文）</Label>
            <Button
              size="sm"
              variant="ghost"
              data-testid="ai-audit-refresh"
              onClick={() =>
                void aiService
                  .auditTail(5)
                  .then(setAudit)
                  .catch((error) => reportFailure('读取审计日志失败', errorMessage(error)))
              }
            >
              <RefreshCw className="h-3 w-3" />
              刷新
            </Button>
          </div>
          {auditError ? (
            <p data-testid="ai-audit-error" className="text-[11px] text-amber-600">
              读取审计失败：{auditError}
            </p>
          ) : audit.length === 0 ? (
            <p className="text-[11px] text-[var(--ink-subtle)]">还没有 AI 行为记录。</p>
          ) : (
            <ul
              data-testid="ai-audit-list"
              className="space-y-0.5 text-[11px] text-[var(--ink-muted)]"
            >
              {audit.map((entry, index) => (
                <li key={`${entry.at}-${index}`} className="flex items-center gap-2">
                  <span className="w-28 shrink-0 tabular-nums text-[var(--ink-subtle)]">
                    {formatDateTime(entry.at)}
                  </span>
                  <span className="w-24 shrink-0">{entry.capability}</span>
                  <span className={entry.outcome === 'ok' ? 'text-emerald-600' : 'text-amber-600'}>
                    {entry.outcome === 'ok'
                      ? '成功'
                      : entry.outcome === 'denied'
                        ? '被拒绝'
                        : '失败'}
                  </span>
                  <span className="truncate text-[var(--ink-subtle)]">
                    {entry.outbound ? '（已外发）' : '（本地）'}
                    {entry.note ? ` ${entry.note}` : ''}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      {/* 决策模型授权确认（D4）：与在线模型同一套口径——说清"内容会离开本机、发给谁" */}
      <Dialog open={decisionAskOpen} onOpenChange={(open) => !open && setDecisionAskOpen(false)}>
        <DialogContent
          className="w-[min(560px,calc(100vw-2rem))]"
          data-testid="ai-decision-confirm"
        >
          <DialogHeader>
            <DialogTitle className="flex items-center gap-1.5">
              <Sparkles className="h-4 w-4 text-[var(--brand)]" />
              启用「决策模型」
            </DialogTitle>
            <DialogDescription>
              开启后，研究助手在「打分与判断」时会把该步骤用到的材料片段发送到下面的端点
              （决策模型只返回概率，不返回文字）。内容会离开本机，请确认该服务商的条款与
              所在地法规要求。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-1.5 text-xs text-[var(--ink-muted)]">
            <p>
              · 目标端点：<code>{policy.decisionBaseUrl ?? 'https://api.perplexity.ai'}</code>
              ，模型 <code>{policy.decisionModel ?? '（按端点预设）'}</code>
            </p>
            <p>· 换地址或换模型名后授权自动作废，需要重新确认一次；</p>
            <p>· 每次外发都会记入审计（不含正文），可在下方「最近行为」里查看；</p>
            <p>· 随时可以切回「规则档」——离线、不联网，永远不会失效。</p>
          </div>
          <DialogFooter>
            <Button
              variant="secondary"
              data-testid="ai-decision-cancel"
              onClick={() => setDecisionAskOpen(false)}
            >
              暂不授权
            </Button>
            <Button
              data-testid="ai-decision-ok"
              onClick={() => {
                setDecisionAskOpen(false);
                void apply({ confirmDecision: true }, '已授权：打分时会把材料片段发给决策端点');
              }}
            >
              确认授权
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 向量索引构建确认（D3）：**先把"要发什么、发给谁"摆出来** */}
      <Dialog open={vectorAskOpen} onOpenChange={(open) => !open && setVectorAskOpen(false)}>
        <DialogContent className="w-[min(560px,calc(100vw-2rem))]" data-testid="ai-vector-confirm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-1.5">
              <Sparkles className="h-4 w-4 text-[var(--brand)]" />
              构建向量索引
            </DialogTitle>
            <DialogDescription>
              这一步会把「将要发送的片段」发给你配置的嵌入端点（一次性动作；此后每次检索只发送查询词）。
              内容会离开本机，请确认该服务商的条款与所在地法规要求。
            </DialogDescription>
          </DialogHeader>
          {vectorPreview ? (
            <div className="space-y-1.5 text-xs text-[var(--ink-muted)]">
              <p>
                · 目标主机：<code>{vectorPreview.host ?? '（未知）'}</code>
              </p>
              <p>
                · 涉及 {vectorPreview.notes} 篇笔记 / {vectorPreview.chunksTotal}{' '}
                个分块；本次实际外发 {vectorPreview.chunksToEmbed} 个片段（增量：内容没变的不重发）
              </p>
              <p>· {vectorPreview.reason}</p>
              <p>· 向量存在本地；随时可以关掉向量检索，回退到纯关键词检索。</p>
            </div>
          ) : null}
          <DialogFooter>
            <Button
              variant="secondary"
              data-testid="ai-vector-cancel"
              onClick={() => setVectorAskOpen(false)}
            >
              暂不构建
            </Button>
            <Button data-testid="ai-vector-ok" onClick={() => void confirmVectorBuild()}>
              仅此一次，开始构建
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={remoteConfirmOpen}
        onOpenChange={(open) => {
          if (open) {
            setRemoteConfirmOpen(true);
            return;
          }
          setRemoteConfirmOpen(false);
          setPendingRemoteSwitch(null);
        }}
      >
        <DialogContent className="w-[min(560px,calc(100vw-2rem))]" data-testid="ai-remote-confirm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-1.5">
              <Sparkles className="h-4 w-4 text-[var(--brand)]" />
              启用「在线模型」
            </DialogTitle>
            <DialogDescription>
              开启后，研究助手在生成报告与打分时，会把该步骤用到的材料片段发送到你填写的地址
              （不会上传整篇知识库）。内容会离开本机，可能被服务商留存或按其政策使用；
              请事先确认该服务商的条款与所在地法规要求。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-1.5 text-xs text-[var(--ink-muted)]">
            <p>· 需要先填好基地址、模型名与 API Key，否则这一档不会生效；</p>
            <p>· 换地址或模型名后授权自动作废，需要重新确认一次；</p>
            <p>· 每次外发都会记入审计（不含正文），可在下方「最近行为」里查看；</p>
            <p>· 随时可以关闭这两个开关，或清除 API Key。</p>
          </div>
          <DialogFooter>
            <Button
              variant="secondary"
              onClick={() => {
                setRemoteConfirmOpen(false);
                setPendingRemoteSwitch(null);
                toast.info('已取消：在线模型保持关闭');
              }}
            >
              暂不启用
            </Button>
            <Button
              data-testid="ai-remote-confirm-ok"
              disabled={busy}
              onClick={() => {
                const target = pendingRemoteSwitch;
                setRemoteConfirmOpen(false);
                setPendingRemoteSwitch(null);
                void apply(
                  {
                    confirmRemoteModel: true,
                    ...(target === 'score' ? { remoteModelScore: true } : {}),
                    ...(target === 'compose' ? { remoteModelCompose: true } : {}),
                  },
                  '已授权在线模型外发（换地址或模型名后需重新确认）',
                );
              }}
            >
              确认启用
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={confirmOpen} onOpenChange={(open) => setConfirmOpen(!open ? false : true)}>
        <DialogContent className="w-[min(560px,calc(100vw-2rem))]" data-testid="ai-network-confirm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-1.5">
              <Sparkles className="h-4 w-4 text-[var(--brand)]" />
              启用「允许联网检索」
            </DialogTitle>
            <DialogDescription>
              开启后，AI 能力在**你每次确认**之后才可以访问公开网页： 会上传的内容是**检索片段 +
              待验证的问题**（不会上传整篇知识库），
              抓取的域名受白/黑名单约束；随时可以切回「仅本地」，切回后正在进行的联网动作会中止。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-1.5 text-xs text-[var(--ink-muted)]">
            <p>· 关闭「抓取网页正文」时只用搜索摘要；</p>
            <p>· 每次研究都有预算上限（子问题 / 抓取页 / 时长）；</p>
            <p>· 所有行为记入审计（不含正文），可在下方「最近行为」里查看。</p>
          </div>
          <DialogFooter>
            <Button
              variant="secondary"
              onClick={() => {
                setConfirmOpen(false);
                toast.info('已取消：联网检索保持「仅本地」');
              }}
            >
              暂不启用
            </Button>
            <Button
              data-testid="ai-network-confirm-ok"
              disabled={busy}
              onClick={() => {
                setConfirmOpen(false);
                void apply(
                  { network: 'allowNetwork', confirmNetwork: true },
                  '已启用联网检索（仅在你确认后才会真正发起请求）',
                );
              }}
            >
              确认启用
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3 px-3 py-2">
      <div className="min-w-0">
        <p className="text-[13px] leading-tight text-[var(--ink)]">{label}</p>
        {hint ? (
          <p className="mt-0.5 text-xs leading-relaxed text-[var(--ink-subtle)]">{hint}</p>
        ) : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

type StatusTone = 'ok' | 'muted' | 'warn';

function KeyStatusBadge({ configured, testId }: { configured: boolean; testId: string }) {
  return (
    <span
      data-testid={testId}
      className={
        configured
          ? 'rounded-full bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300'
          : 'rounded-full bg-[var(--surface-sunken)] px-2 py-0.5 text-[11px] font-medium text-[var(--ink-subtle)]'
      }
    >
      {configured ? '已配置 Key' : '未配置 Key'}
    </span>
  );
}

function StatusPill({ tone, label }: { tone: StatusTone; label: string }) {
  const toneClass =
    tone === 'ok'
      ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
      : tone === 'warn'
        ? 'bg-amber-500/10 text-amber-700 dark:text-amber-300'
        : 'bg-[var(--surface-sunken)] text-[var(--ink-subtle)]';
  return (
    <span className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${toneClass}`}>{label}</span>
  );
}

function StatusCard({
  label,
  value,
  tone,
  onClick,
}: {
  label: string;
  value: string;
  tone: StatusTone;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={`${label}：${value}`}
      onClick={onClick}
      className="group min-h-14 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)] px-3 py-2.5 text-left transition-[border-color,background-color,box-shadow] hover:border-[var(--brand)] hover:bg-[var(--surface-muted)]/45 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]"
    >
      <div className="flex items-center justify-between gap-2">
        <p className="text-[11px] font-medium text-[var(--ink-subtle)]">{label}</p>
        <ChevronRight className="h-3.5 w-3.5 text-[var(--ink-subtle)] transition-transform group-hover:translate-x-0.5 group-hover:text-[var(--brand)]" />
      </div>
      <div className="mt-1 flex items-center gap-1.5">
        <span
          className={cn(
            'h-2 w-2 shrink-0 rounded-full',
            tone === 'ok'
              ? 'bg-emerald-500'
              : tone === 'warn'
                ? 'bg-amber-500'
                : 'bg-[var(--ink-subtle)]',
          )}
        />
        <p className="truncate text-xs font-medium text-[var(--ink)]" title={value}>
          {value}
        </p>
      </div>
    </button>
  );
}

function SectionHeading({
  id,
  title,
  description,
}: {
  id: string;
  title: string;
  description: string;
}) {
  return (
    <div id={id} className="scroll-mt-16 px-3 pb-1 pt-3">
      <h3 className="text-sm font-semibold text-[var(--ink)]">{title}</h3>
      <p className="mt-0.5 text-xs leading-relaxed text-[var(--ink-subtle)]">{description}</p>
    </div>
  );
}

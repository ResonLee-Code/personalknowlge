import { Loader2, RefreshCw, Sparkles } from 'lucide-react';
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
  const [vectorProgress, setVectorProgress] = useState<{ done: number; total: number } | null>(null);
  const [embeddingDraft, setEmbeddingDraft] = useState('');
  /** 决策模型（D4）：配置草稿 + 探针结果 */
  const [decisionBaseDraft, setDecisionBaseDraft] = useState('');
  const [decisionModelDraft, setDecisionModelDraft] = useState('');
  const [decisionKeyDraft, setDecisionKeyDraft] = useState('');
  const [decisionAskOpen, setDecisionAskOpen] = useState(false);
  const [probe, setProbe] = useState<DecisionProbeReport | null>(null);
  const [probing, setProbing] = useState(false);
  const [proxyDraft, setProxyDraft] = useState('');
  const [loadError, setLoadError] = useState<string | null>(null);

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

  const apply = async (input: Parameters<typeof aiService.setPolicy>[0], message?: string) => {
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
    } catch (error) {
      reportFailure('读取审计日志失败', errorMessage(error));
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
    try {
      setProbe(await aiService.decisionProbe());
    } catch (error) {
      reportFailure('决策探针失败', errorMessage(error));
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

  return (
    <div className="divide-y divide-[var(--border-subtle)]">
      <Row label="AI 能力总开关" hint="默认关闭；关闭时所有 AI 命令都会被后端拒绝">
        <Switch
          data-testid="ai-enabled"
          checked={policy.enabled}
          onCheckedChange={(checked) =>
            void apply({ enabled: checked }, checked ? '已开启 AI 能力（仍不会自动联网）' : '已关闭 AI 能力')
          }
        />
      </Row>

      <Row label="模型自检" hint="一键看「写报告」现在用哪一档，以及为什么（只读，不联系在线端点）">
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

      <Row label="联网检索" hint="仅本地：只用本地模型与本地检索；允许联网后仍需每次确认">
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
            应用不预置任何第三方检索服务：这里填你自己可用的检索接口（返回 JSON，
            支持 SearxNG 的 {'{ "results": [ … ] }'} 形状）。留空表示不联网检索；
            地址不合法或缺少 {'{query}'} 会被清空。
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
                void apply({ vectorEnabled: true }, '已开启向量检索（还需填嵌入模型名并确认一次）');
                return;
              }
              void apply({ vectorEnabled: checked });
            }}
          />
        </div>
        <Input
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
              void apply({ confirmVector: true }, '已授权：查询词会发给嵌入端点（换地址/换模型后需重新确认）')
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
            <span data-testid="ai-vector-progress" className="text-[11px] text-[var(--ink-subtle)]">
              已嵌 {vectorProgress.done} / {vectorProgress.total}
            </span>
          ) : null}
        </div>
        <p className="text-[11px] text-[var(--ink-subtle)]">
          只能增强召回，「不能」决定检索是否可用：向量档不可用时自动只用关键词检索。
          构建是增量且需确认的一次性动作（只有内容变过的片段才会重发）。
        </p>
      </div>

      <div className="space-y-1.5 px-3 py-2">
        <Label>本地模型（Ollama HTTP，可选；默认关闭）</Label>
        <div className="space-y-1">
          <Input
            aria-label="本地模型地址"
            data-testid="ai-local-endpoint"
            defaultValue={policy.localModelEndpoint ?? ''}
            placeholder="http://127.0.0.1:11434"
            onBlur={(event) =>
              void apply({ localModelEndpoint: event.target.value }, '已保存本地模型地址')
            }
          />
          <Input
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
        <Row label="用本地模型打分" hint="与规则档 A/B 用；模型分未校准，级别折算仍占一半（不是 Jev）">
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
      <div className="space-y-1.5 px-3 py-2">
        <div className="flex items-center justify-between">
          <Label>在线模型（OpenAI 兼容端点，可选；默认关闭）</Label>
          <span
            data-testid="ai-remote-key-badge"
            className={
              policy.remoteModelKeySet
                ? 'rounded-full bg-emerald-500/10 px-2 py-0.5 text-[11px] text-emerald-600'
                : 'rounded-full bg-[var(--surface-sunken)] px-2 py-0.5 text-[11px] text-[var(--ink-subtle)]'
            }
          >
            {policy.remoteModelKeySet ? '已配置 Key' : '未配置 Key'}
          </span>
        </div>
        <div className="space-y-1">
          <Input
            aria-label="在线模型地址"
            data-testid="ai-remote-endpoint"
            defaultValue={policy.remoteModelBaseUrl ?? ''}
            placeholder="https://api.example.com/v1（必须是 https 的公网域名）"
            onBlur={(event) =>
              void apply({ remoteModelBaseUrl: event.target.value }, '已保存在线模型地址')
            }
          />
          <Input
            aria-label="在线模型名称"
            data-testid="ai-remote-model"
            defaultValue={policy.remoteModelName ?? ''}
            placeholder="deepseek-chat（服务商提供的模型名）"
            onBlur={(event) => void apply({ remoteModelName: event.target.value }, '已保存模型名')}
          />
          <div className="flex items-center gap-2">
            <Input
              aria-label="在线模型 API Key"
              data-testid="ai-remote-key"
              type="password"
              autoComplete="off"
              value={remoteKeyDraft}
              placeholder="API Key（只写不读，保存后无法再查看）"
              onChange={(event) => setRemoteKeyDraft(event.target.value)}
            />
            <Button
              size="sm"
              variant="secondary"
              data-testid="ai-remote-key-save"
              disabled={busy || remoteKeyDraft.trim().length === 0}
              onClick={() => {
                const key = remoteKeyDraft;
                setRemoteKeyDraft('');
                void apply({ remoteModelApiKey: key }, '已保存 API Key（存储在本机应用数据目录）');
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
        <Row
          label="用在线模型写报告"
          hint="材料片段会离开本机；失败自动降级到本地模型或模板实现"
        >
          <Switch
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
            : '尚未授权：这一档不会向任何地址发送内容。'}
        </p>
        <p className="text-[11px] text-[var(--ink-subtle)]">
          给「本机跑不动本地模型」的机器留的一档：地址只接受 https 的公网域名（本机可写
          localhost），裸 IP 与非加密地址会被拒绝。Key 存在本机应用数据目录（不进工作区、
          不进备份）；本节的外发由独立授权控制，与上面的「联网检索」档位互不影响。
        </p>
      </div>

      {/* 决策模型（D4）：规则档（默认，离线）/ System One（Jev 或 Perplexity Decisions，同协议） */}
      <div className="space-y-1.5 px-3 py-2">
        <Label>决策模型（打分 / 判断用哪一档）</Label>
        <select
          aria-label="决策模型档位"
          data-testid="ai-decision-provider"
          value={policy.decisionProvider ?? 'rule'}
          onChange={(event) => {
            const next = event.target.value as 'rule' | 'system_one';
            void apply(
              { decisionProvider: next },
              next === 'rule' ? '已切回规则档' : '已选择 System One（还需填端点、模型与 Key 并授权）',
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
            <Input
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
            <Input
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
            <div className="flex flex-wrap items-center gap-2">
              <Input
                aria-label="决策模型 API Key"
                data-testid="ai-decision-key"
                type="password"
                value={decisionKeyDraft}
                onChange={(event) => setDecisionKeyDraft(event.target.value)}
                placeholder={policy.decisionKeySet ? '已配置（只写不读，重填可覆盖）' : 'API Key（只写不读）'}
              />
              <Button
                size="sm"
                variant="secondary"
                data-testid="ai-decision-key-save"
                onClick={() => {
                  void apply({ decisionApiKey: decisionKeyDraft }, '已保存决策模型 Key');
                  setDecisionKeyDraft('');
                }}
              >
                保存 Key
              </Button>
              <Button
                size="sm"
                variant="ghost"
                data-testid="ai-decision-key-clear"
                onClick={() => void apply({ decisionApiKey: '' }, '已清除决策模型 Key')}
              >
                清除
              </Button>
            </div>
            <p data-testid="ai-decision-authorized" className="text-[11px] text-[var(--ink-subtle)]">
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
                  A/B（{probe.ab.cases} 条打分，相关/不相关各半）：规则档{' '}
                  {probe.ab.rule.correct}/{probe.ab.rule.total}（区分度{' '}
                  {probe.ab.rule.separation.toFixed(3)}）· 决策档{' '}
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
      <div className="space-y-1.5 px-3 py-2">
        <Label>出站代理（留空 = 跟随系统代理）</Label>
        <Input
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

      <div className="space-y-1.5 px-3 py-2">
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
          <ul data-testid="ai-audit-list" className="space-y-0.5 text-[11px] text-[var(--ink-muted)]">
            {audit.map((entry, index) => (
              <li key={`${entry.at}-${index}`} className="flex items-center gap-2">
                <span className="w-28 shrink-0 tabular-nums text-[var(--ink-subtle)]">
                  {formatDateTime(entry.at)}
                </span>
                <span className="w-24 shrink-0">{entry.capability}</span>
                <span className={entry.outcome === 'ok' ? 'text-emerald-600' : 'text-amber-600'}>
                  {entry.outcome === 'ok' ? '成功' : entry.outcome === 'denied' ? '被拒绝' : '失败'}
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

      {/* 决策模型授权确认（D4）：与在线模型同一套口径——说清"内容会离开本机、发给谁" */}
      <Dialog open={decisionAskOpen} onOpenChange={(open) => !open && setDecisionAskOpen(false)}>
        <DialogContent className="w-[min(560px,calc(100vw-2rem))]" data-testid="ai-decision-confirm">
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
        <DialogContent
          className="w-[min(560px,calc(100vw-2rem))]"
          data-testid="ai-vector-confirm"
        >
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
                · 涉及 {vectorPreview.notes} 篇笔记 / {vectorPreview.chunksTotal} 个分块；本次实际外发{' '}
                {vectorPreview.chunksToEmbed} 个片段（增量：内容没变的不重发）
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
        <DialogContent
          className="w-[min(560px,calc(100vw-2rem))]"
          data-testid="ai-remote-confirm"
        >
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
              开启后，AI 能力在**你每次确认**之后才可以访问公开网页：
              会上传的内容是**检索片段 + 待验证的问题**（不会上传整篇知识库），
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
          <p className="mt-0.5 text-[11px] leading-tight text-[var(--ink-subtle)]">{hint}</p>
        ) : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

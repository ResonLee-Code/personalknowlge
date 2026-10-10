import { Loader2, RefreshCw, Sparkles } from 'lucide-react';
import { useEffect, useState } from 'react';
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
import { errorMessage, formatDateTime } from '@/lib/format';
import { aiService } from '@/services/aiService';
import type { AiAuditEntry, AiPolicy } from '@/types';

/**
 * 「设置 → AI」区块（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.7）。
 *
 * 默认口径：总开关**关闭**、联网档位**仅本地**、抓取正文**关闭**（既定决策，2026-10-10 确认）。
 * 切到「允许联网检索」时必须先看一次说明并确认；判定逻辑全在后端闸门，这里只做展示与提交。
 */
export function AiSettingsPanel() {
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [audit, setAudit] = useState<AiAuditEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [denylistDraft, setDenylistDraft] = useState('');
  const [allowlistDraft, setAllowlistDraft] = useState('');

  useEffect(() => {
    let cancelled = false;
    void aiService
      .policy()
      .then((next) => {
        if (cancelled) return;
        setPolicy(next);
        setDenylistDraft(next.domainDenylist.join('\n'));
        setAllowlistDraft(next.domainAllowlist.join('\n'));
      })
      .catch(() => {
        // 未打开工作区等场景：静默降级为「不可用」，不打断设置页
      });
    void aiService
      .auditTail(5)
      .then((entries) => {
        if (!cancelled) setAudit(entries);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const apply = async (input: Parameters<typeof aiService.setPolicy>[0], message?: string) => {
    setBusy(true);
    try {
      const next = await aiService.setPolicy(input);
      setPolicy(next);
      if (message) toast.success(message);
      const entries = await aiService.auditTail(5).catch(() => []);
      setAudit(entries);
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const parseLines = (value: string) =>
    value
      .split(/[\n,]/)
      .map((item) => item.trim())
      .filter(Boolean);

  if (!policy) {
    return (
      <div className="flex items-center gap-2 px-3 py-3 text-xs text-[var(--ink-subtle)]">
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

      <Row label="联网检索" hint="仅本地：只用本地模型与本地检索；允许联网后仍需每次确认">
        <select
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

      <div className="space-y-2 px-3 py-2.5">
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

      <div className="space-y-2 px-3 py-2.5">
        <Label>本地模型（Ollama HTTP，可选；默认关闭）</Label>
        <div className="space-y-1">
          <Input
            data-testid="ai-local-endpoint"
            defaultValue={policy.localModelEndpoint ?? ''}
            placeholder="http://127.0.0.1:11434"
            onBlur={(event) =>
              void apply({ localModelEndpoint: event.target.value }, '已保存本地模型地址')
            }
          />
          <Input
            data-testid="ai-local-model"
            defaultValue={policy.localModelName ?? ''}
            placeholder="qwen2.5:7b（模型名，需先 ollama pull）"
            onBlur={(event) => void apply({ localModelName: event.target.value }, '已保存模型名')}
          />
        </div>
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
          只连本机地址（localhost / 127.0.0.1）；远程模型需要单独授权，本版不做。
          两项开关都关闭时一个请求都不会发；浏览器预览模式不会真的调用本地模型。
        </p>
      </div>

      <div className="space-y-2 px-3 py-2.5">
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

      <div className="space-y-1.5 px-3 py-2.5">
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
                .catch((error) => toast.error(errorMessage(error)))
            }
          >
            <RefreshCw className="h-3 w-3" />
            刷新
          </Button>
        </div>
        {audit.length === 0 ? (
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
    <div className="flex items-center justify-between gap-4 px-3 py-2.5">
      <div className="min-w-0">
        <p className="text-sm text-[var(--ink)]">{label}</p>
        {hint ? <p className="mt-0.5 text-[11px] text-[var(--ink-subtle)]">{hint}</p> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

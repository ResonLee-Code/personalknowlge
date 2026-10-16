import {
  Database,
  FolderOpen,
  HardDrive,
  PenLine,
  RefreshCw,
  Search,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Input, Label } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { AiSettingsPanel } from '@/features/settings/AiSettingsPanel';
import { reportAction, reportFailure } from '@/lib/actionFeedback';
import { AUTOSAVE_DELAY_OPTIONS, FONT_SIZE_OPTIONS, ROUTES, THEME_OPTIONS } from '@/lib/constants';
import { cn } from '@/lib/utils';
import { logAppFailure } from '@/lib/appLog';
import { errorMessage, formatBytes, formatDateTime } from '@/lib/format';
import { formatWarningsToast } from '@/lib/warnings';
import { backupService } from '@/services/backupService';
import { openPath, pickDirectory } from '@/services/appService';
import { isTauriRuntime } from '@/services/api';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { BackupInfo, BackupValidation } from '@/types';

/**
 * 设置页的分节与左侧导航（2026-10-16）。
 *
 * 结构照 Codex 的设置页来：左边是"标题 + 搜索 + 分组导航"，右边只显示**当前分节**；
 * 搜索时右边把所有分节都摆出来，只留命中的行（分节整体不命中就整节藏掉）。
 * `keywords` 是这一节行的关键词集合，供搜索判断"这一节还有没有命中"。
 */
type SettingsSectionId = 'editor' | 'index' | 'backup' | 'ai' | 'about';

const SETTINGS_GROUPS: Array<{
  label: string;
  items: Array<{ id: SettingsSectionId; label: string; icon: typeof Database; keywords: string }>;
}> = [
  {
    label: '个人',
    items: [
      {
        id: 'editor',
        label: '编辑器',
        icon: PenLine,
        keywords: '自动保存 延迟 默认视图 字号 主题 新建位置 大纲位置 大纲 自动显示大纲',
      },
      { id: 'ai', label: 'AI 能力', icon: Sparkles, keywords: 'AI 联网 模型 本地 在线 向量 决策 代理 审计' },
    ],
  },
  {
    label: '数据',
    items: [
      { id: 'index', label: '索引与数据', icon: Database, keywords: '索引 重建 扫描 工作区目录' },
      { id: 'backup', label: '备份与导出', icon: HardDrive, keywords: '备份 校验 恢复 导出 Markdown' },
    ],
  },
  { label: '关于', items: [{ id: 'about', label: '关于本应用', icon: ShieldCheck, keywords: '版本 数据 日志 运行模式 工作区 ID' }] },
];

/** 设置页面（设计文档 16.7）。 */
export function SettingsPage() {
  const settings = useWorkspaceStore((state) => state.settings);
  const workspace = useWorkspaceStore((state) => state.workspace);
  const updateSettings = useWorkspaceStore((state) => state.updateSettings);
  const rebuildIndex = useWorkspaceStore((state) => state.rebuildIndex);
  const scan = useWorkspaceStore((state) => state.scan);
  const indexStatus = useWorkspaceStore((state) => state.indexStatus);
  const theme = useUiStore((state) => state.theme);
  const setTheme = useUiStore((state) => state.setTheme);
  const setOutlinePlacement = useUiStore((state) => state.setOutlinePlacement);
  const setAutoOutlineOnSelect = useUiStore((state) => state.setAutoOutlineOnSelect);

  /** 当前选中的设置分节（左侧导航）与搜索词 */
  const [section, setSection] = useState<SettingsSectionId>('editor');
  const [query, setQuery] = useState('');
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  /** 备份列表读取失败的原因：失败不能显示成「暂无备份文件」（那会让人以为备份没了） */
  const [backupListError, setBackupListError] = useState<string | null>(null);
  const [validation, setValidation] = useState<Record<string, BackupValidation | null>>({});
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void backupService
      .list()
      .then((items) => {
        setBackups(items);
        setBackupListError(null);
      })
      .catch((error: unknown) => {
        setBackups([]);
        setBackupListError(errorMessage(error));
        void logAppFailure('backup_list', error, 'warn');
      });
  }, []);

  if (!settings) {
    return <p className="p-6 text-sm text-[var(--ink-subtle)]">正在加载设置…</p>;
  }

  const createBackup = async () => {
    setBusy(true);
    try {
      const info = await backupService.create();
      setBackups(await backupService.list());
      setBackupListError(null);
      toast.success(`备份完成：${info.fileName}`);
    } catch (error) {
      reportFailure('备份失败', errorMessage(error));
      void logAppFailure('backup_create', error);
    } finally {
      setBusy(false);
    }
  };

  const validate = async (path: string) => {
    try {
      const result = await backupService.validate(path);
      setValidation((previous) => ({ ...previous, [path]: result }));
      toast[result.valid ? 'success' : 'error'](
        result.valid ? '校验通过' : '校验未通过，请查看详情',
      );
    } catch (error) {
      reportFailure('校验失败', errorMessage(error));
      void logAppFailure('backup_validate', error);
    }
  };

  const restore = async (path: string) => {
    const target = isTauriRuntime()
      ? await pickDirectory('选择恢复目标目录（必须为空）')
      : window.prompt('输入恢复目标目录（必须为空）');
    if (!target) return;
    try {
      const report = await backupService.restore(path, target);
      toast.success(`已恢复 ${report.extractedFiles} 个文件到 ${report.targetDir}`);
    } catch (error) {
      reportFailure('恢复失败', errorMessage(error));
      void logAppFailure('backup_restore', error);
    }
  };

  const exportMarkdown = async () => {
    const target = isTauriRuntime()
      ? await pickDirectory('选择导出目录（必须为空）')
      : window.prompt('输入导出目录（必须为空）');
    if (!target) return;
    try {
      const result = await backupService.exportMarkdown(target);
      toast.success(`已导出 ${result.noteCount} 篇笔记与 ${result.attachmentCount} 个附件`);
    } catch (error) {
      reportFailure('导出失败', errorMessage(error));
      void logAppFailure('export_markdown', error);
    }
  };

  return (
    <SettingsSearchContext.Provider value={query}>
      <div className="flex h-full min-h-0">
        {/* 左侧导航（2026-10-16，按 Codex 设置页的样式）：标题 + 搜索 + 分组导航 */}
        <nav
          aria-label="设置导航"
          className="flex w-56 shrink-0 flex-col gap-3 overflow-auto border-r border-[var(--border-subtle)] px-3 py-4"
        >
          <div className="px-2">
            <h1 className="text-base font-semibold text-[var(--ink)]">设置</h1>
            <p className="mt-0.5 text-[11px] leading-snug text-[var(--ink-subtle)]">
              保存在工作区数据库的 <code>settings</code> 表，切换工作区互不影响
            </p>
          </div>
          <div className="relative px-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-subtle)]" />
            <Input
              aria-label="搜索设置"
              placeholder="搜索"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              className="h-8 pl-8 text-xs"
            />
          </div>
          {SETTINGS_GROUPS.map((group) => (
            <div key={group.label} className="space-y-0.5">
              <p className="px-2 text-[11px] font-medium text-[var(--ink-subtle)]">{group.label}</p>
              {group.items.map((item) => {
                const isActive = item.id === section;
                const dimmed = Boolean(query) && !matches(query, item.label, item.keywords);
                return (
                  <button
                    key={item.id}
                    type="button"
                    data-testid={`settings-nav-${item.id}`}
                    aria-current={isActive ? 'true' : undefined}
                    disabled={dimmed}
                    onClick={() => {
                      setSection(item.id);
                      setQuery('');
                    }}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] transition-colors',
                      isActive
                        ? 'bg-[var(--surface-muted)] font-medium text-[var(--ink)]'
                        : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]',
                      dimmed && 'opacity-40',
                    )}
                  >
                    <item.icon className="h-3.5 w-3.5 shrink-0" />
                    <span className="truncate">{item.label}</span>
                  </button>
                );
              })}
            </div>
          ))}
        </nav>

        <div data-testid="settings-content" className="min-w-0 flex-1 overflow-auto px-6 py-5">
          {/* 右侧内容**铺满**（2026-10-17）：以前限宽 max-w-3xl，宽窗口下右边空一大片 */}
          <div className="w-full">

        {/* 瀑布流（2026-10-16 重排）：卡片按高度紧挨着落进各列——
            以前是"两列网格 + items-start"，右边那列列高很多，左边底部就空出一大片，
            整体看着松。列数随窗口宽窄自适应（窄屏 1 列、常规 2 列、很宽 3 列）。 */}
        <div className="columns-1 gap-4 xl:columns-2 2xl:columns-3">
        {section === 'editor' || query ? (
        <Section title="编辑器" icon={Database}
          searchText="自动保存 延迟 默认视图 字号 界面主题 新建笔记位置 大纲位置 大纲 选中笔记后自动显示大纲"
        >
          <Row label="自动保存" hint="停止输入后自动写入文件（默认 1000ms）">
            <Switch
              checked={settings.autosaveEnabled}
              onCheckedChange={(checked) => void updateSettings({ autosaveEnabled: checked })}
            />
          </Row>
          <Row label="自动保存延迟" hint="200ms ~ 10000ms">
            <select
              aria-label="自动保存延迟"
              value={settings.autosaveDelayMs}
              onChange={(event) =>
                void updateSettings({ autosaveDelayMs: Number(event.target.value) })
              }
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              {AUTOSAVE_DELAY_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option} ms
                </option>
              ))}
            </select>
          </Row>
          <Row
            label="默认视图"
            hint="打开笔记时的默认布局；在编辑器顶部切换后会被记住"
          >
            <select
              aria-label="默认视图"
              value={settings.editorMode}
              onChange={(event) => void updateSettings({ editorMode: event.target.value })}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              <option value="edit">编辑</option>
              <option value="split">分屏</option>
              <option value="preview">预览</option>
            </select>
          </Row>
          <Row label="字号" hint="编辑器与预览的字号">
            <select
              aria-label="字号"
              value={settings.fontSize}
              onChange={(event) => void updateSettings({ fontSize: Number(event.target.value) })}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              {FONT_SIZE_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option}px
                </option>
              ))}
            </select>
          </Row>
          <Row label="界面主题" hint="浅色 / 深色">
            <select
              aria-label="界面主题"
              value={theme}
              onChange={(event) => {
                const value = event.target.value as 'light' | 'dark';
                setTheme(value);
                void updateSettings({ theme: value });
              }}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              {THEME_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Row>
          <Row label="新建笔记位置" hint="Inbox 或最近使用的分类">
            <select
              aria-label="新建笔记位置"
              value={settings.defaultNewLocation}
              onChange={(event) =>
                void updateSettings({ defaultNewLocation: event.target.value })
              }
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              <option value="inbox">Inbox</option>
              <option value="lastCategory">上次使用的分类</option>
            </select>
          </Row>
          <Row
            label="大纲位置"
            hint="与笔记列表共用一栏（省横向空间，默认）或钉在正文右侧；编辑器工具栏也能一键切换"
          >
            <select
              aria-label="大纲位置"
              value={settings.outlinePlacement}
              onChange={(event) => {
                const value = event.target.value as 'list' | 'side';
                setOutlinePlacement(value);
                void updateSettings({ outlinePlacement: value });
              }}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              <option value="list">与笔记列表共用一栏</option>
              <option value="side">钉在正文右侧</option>
            </select>
          </Row>
          <Row
            label="选中笔记后自动显示大纲"
            hint="默认关：浏览列表时连着点几篇不会被切走；打开后选中即看结构"
          >
            <Switch
              checked={settings.autoOutlineOnSelect}
              onCheckedChange={(checked) => {
                setAutoOutlineOnSelect(checked);
                void updateSettings({ autoOutlineOnSelect: checked });
              }}
            />
          </Row>
        </Section>
        ) : null}

        {section === 'index' || query ? (
        <Section title="索引与数据" icon={ShieldCheck}
          searchText="索引状态 重建索引 重新扫描工作区 工作区目录"
        >
          <Row label="索引状态" hint={`${indexStatus?.indexedNotes ?? 0} / ${indexStatus?.totalNotes ?? 0} 篇已索引`}>
            <span className="text-xs text-[var(--ink-muted)]">{indexStatus?.state ?? 'ready'}</span>
          </Row>
          <Row label="重建索引" hint="清空并重新扫描全部 Markdown 文件">
            <Button
              size="sm"
              variant="secondary"
              onClick={() =>
                void rebuildIndex().then((ok) =>
                  reportAction(ok, '索引已重建', useWorkspaceStore.getState().error, '重建索引失败'),
                )
              }
            >
              <RefreshCw className="h-3.5 w-3.5" />
              重建
            </Button>
          </Row>
          <Row label="重新扫描工作区" hint="采纳外部新增或修改的文件">
            <Button
              size="sm"
              variant="secondary"
              onClick={() =>
                void scan().then((report) => {
                  if (!report) {
                    reportFailure('扫描失败', useWorkspaceStore.getState().error);
                    return;
                  }
                  toast.success(
                    `新增 ${report.added}，更新 ${report.updated}，缺失 ${report.missing}`,
                  );
                  const notice = formatWarningsToast(report.warnings);
                  if (notice) toast.warning(notice);
                })
              }
            >
              <RefreshCw className="h-3.5 w-3.5" />
              扫描
            </Button>
          </Row>
          <Row label="工作区目录" hint={workspace?.rootPath ?? '—'}>
            <Button
              size="sm"
              variant="secondary"
              onClick={() =>
                void openPath(workspace?.rootPath ?? '').catch((error: unknown) => {
                  // F2：原因（含系统返回的原文）要点得开，而不是只有一个"无法打开"
                  reportFailure('无法打开目录', errorMessage(error));
                  void logAppFailure('open_path', error, 'warn');
                })
              }
            >
              <FolderOpen className="h-3.5 w-3.5" />
              打开
            </Button>
          </Row>
        </Section>
        ) : null}

        {section === 'backup' || query ? (
        <Section title="备份与导出" icon={HardDrive}
          searchText="备份目录 创建备份 备份列表 校验 恢复 导出 Markdown"
        >
          <Row label="备份目录" hint={settings.backupDir ?? '默认：.knowledge/backups'}>
            <Button
              size="sm"
              variant="secondary"
              onClick={async () => {
                const dir = isTauriRuntime() ? await pickDirectory('选择备份目录') : window.prompt('输入备份目录');
                if (dir) void updateSettings({ backupDir: dir });
              }}
            >
              修改
            </Button>
          </Row>
          <Row label="创建备份" hint="打包 Markdown、附件、数据库与配置">
            <Button size="sm" onClick={() => void createBackup()} disabled={busy}>
              立即备份
            </Button>
          </Row>
          <Row label="导出 Markdown" hint="把全部笔记导出为普通目录">
            <Button size="sm" variant="secondary" onClick={() => void exportMarkdown()}>
              导出
            </Button>
          </Row>
          <div className="space-y-2 px-3 py-2">
            {backupListError ? (
              <p data-testid="backup-list-error" className="text-xs text-amber-600">
                读取备份列表失败：{backupListError}
              </p>
            ) : backups.length === 0 ? (
              // 「校验 / 恢复」是逐条渲染在备份卡片下方的按钮：没有备份时它们不存在。
              // 真机验收时你我都因为看不到这两个入口而各花了时间找（Track B2），所以空态说清楚。
              <p data-testid="backup-empty" className="text-xs text-[var(--ink-subtle)]">
                暂无备份文件。点上方「立即备份」创建后，每份备份下方会出现「校验 / 恢复」。
              </p>
            ) : (
              backups.map((backup) => (
                <div
                  key={backup.path}
                  className="rounded-lg border border-[var(--border-subtle)] p-2.5"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="min-w-0 flex-1 truncate text-xs font-medium text-[var(--ink)]">
                      {backup.fileName}
                    </span>
                    <span className="text-[11px] text-[var(--ink-subtle)]">
                      {formatBytes(backup.sizeBytes)}
                    </span>
                  </div>
                  <div className="mt-0.5 text-[11px] text-[var(--ink-subtle)]">
                    {formatDateTime(backup.createdAt)} · {backup.noteCount} 篇笔记 ·{' '}
                    {backup.includesDatabase ? '含数据库' : '不含数据库'}
                  </div>
                  <div className="mt-1.5 flex gap-2">
                    <Button size="sm" variant="ghost" onClick={() => void validate(backup.path)}>
                      校验
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => void restore(backup.path)}>
                      恢复
                    </Button>
                  </div>
                  {validation[backup.path] ? (
                    <ul className="mt-1.5 space-y-0.5 text-[11px] text-[var(--ink-subtle)]">
                      {validation[backup.path]?.checks.map((check) => (
                        <li key={check.name} className={check.passed ? '' : 'text-red-500'}>
                          {check.passed ? '✓' : '✗'} {check.name}：{check.detail}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </div>
              ))
            )}
          </div>
        </Section>
        ) : null}

        {section === 'ai' || query ? (
        <Section title="AI（第二代预留）" icon={Sparkles} testId="settings-ai-section"
          searchText="AI 总开关 模型自检 联网检索 抓取正文 域名白名单 黑名单 检索源 研究预算 本地模型 在线模型 向量检索 决策模型 出站代理 最近行为"
        >
          <AiSettingsPanel />
        </Section>
        ) : null}

        {section === 'about' || query ? (
        <Section title="关于" icon={ShieldCheck}
          searchText="版本与运行信息 工作区 ID 目录"
        >
          <Row label="版本与运行信息" hint="包含应用版本、数据库版本与日志位置">
            <Button asChild size="sm" variant="secondary">
              <Link to={ROUTES.about}>查看</Link>
            </Button>
          </Row>
          <div className="px-3 py-2">
            <Label className="text-[11px] text-[var(--ink-subtle)]">
              工作区 ID：{workspace?.id ?? '—'}
            </Label>
            <Input
              aria-label="当前知识库目录" readOnly value={workspace?.rootPath ?? ''} className="mt-1 h-8 text-xs" />
          </div>
        </Section>
        ) : null}
        </div>
          </div>
        </div>
      </div>
    </SettingsSearchContext.Provider>
  );
}

function Section({
  title,
  icon: Icon,
  searchText,
  children,
  testId,
}: {
  title: string;
  icon: typeof Database;
  /** 供设置页搜索用的关键词（这一节所有行标题 + 说明的集合） */
  searchText?: string;
  children: ReactNode;
  /** 供组件测试 / E2E 定位该区块 */
  testId?: string;
}) {
  const query = useSettingsSearch();
  // 搜索时：整节都不匹配就把这一节藏掉（否则会留下一个空卡片，看着像坏了）
  if (query && searchText && !matches(query, searchText)) return null;
  return (
    <section data-testid={testId} className="mb-6 break-inside-avoid">
      {/* 分节标题在卡片**外面**（2026-10-16 按 Codex 设置页的样式）：标题属于页面结构，
          卡片只装行；这样一眼能看出"哪几行是一组" */}
      <header className="mb-2 flex items-center gap-1.5">
        <Icon className="h-3.5 w-3.5 text-[var(--brand)]" />
        <h2 className="text-[13px] font-semibold text-[var(--ink)]">{title}</h2>
      </header>
      <div className="divide-y divide-[var(--border-subtle)] rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)]">
        {children}
      </div>
    </section>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  const query = useSettingsSearch();
  // 搜索时只显示命中的行（标题与说明都参与匹配）
  if (query && !matches(query, label, hint)) return null;
  return (
    // 行高收紧：标签与说明都压到 leading-tight，内边距 py-2（原来 2.5 + 两条单倍行距 ≈ 62px，
    // 现在 ≈ 44px）；说明仍然完整保留，只是不再撑出额外空白。
    <div className="flex items-center justify-between gap-3 px-3 py-2">
      <div className="min-w-0">
        <p className="text-[13px] leading-tight text-[var(--ink)]">{label}</p>
        {hint ? (
          <p className="mt-0.5 truncate text-[11px] leading-tight text-[var(--ink-subtle)]">{hint}</p>
        ) : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/** 设置页的搜索词（空串 = 不过滤）；放在 context 里，行与分节各自按它决定显不显示。 */
const SettingsSearchContext = createContext('');
const useSettingsSearch = () => useContext(SettingsSearchContext);

/** 关键词包含匹配（大小写不敏感；中文直接子串）。 */
function matches(query: string, ...texts: Array<string | undefined>): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return texts.some((text) => (text ?? '').toLowerCase().includes(needle));
}

import { Crosshair, ExternalLink, Maximize2, Network, RefreshCw, Unlink } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Skeleton } from '@/components/ui/badge';
import { ROUTES } from '@/lib/constants';
import { errorMessage, formatCount } from '@/lib/format';
import { communityColor, communitySizes, detectCommunities } from '@/lib/graphCommunity';
import { toBase64Utf8 } from '@/lib/graphExport';
import { categoryColor } from '@/lib/graphColors';
import { cn } from '@/lib/utils';
import { attachmentService } from '@/services/attachmentService';
import { graphService } from '@/services/graphService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { GraphData } from '@/types';

import { ForceGraph } from './ForceGraph';

/** 知识图谱视图（V1.1 M3 / 设计文档 27 章）。 */
export function GraphPage() {
  const navigate = useNavigate();
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const tags = useTagsStore((state) => state.tags);
  const currentNoteId = useNotesStore((state) => state.selectedId ?? state.detail?.note.id ?? null);
  /** 当前知识库 id：切换知识库后图谱需要重新查询（V1.2.1 修复） */
  const workspaceId = useWorkspaceStore((state) => state.workspace?.id ?? null);

  const [data, setData] = useState<GraphData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [categoryId, setCategoryId] = useState('');
  const [tagId, setTagId] = useState('');
  const [includeOrphans, setIncludeOrphans] = useState(true);
  /** 时间过滤：''=全部，7/30/90=最近 N 天（按 updatedAt 过滤） */
  const [recent, setRecent] = useState('');
  const [focusId, setFocusId] = useState<string | null>(null);
  const [hops, setHops] = useState(1);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** 着色模式：按分类，或按社区（连通分量） */
  const [colorMode, setColorMode] = useState<'category' | 'community'>('category');
  /** 社区粒度（模块度分辨率）：越大社区越细 */
  const [resolution, setResolution] = useState(1);
  /** 导出时是否标注全部节点标题 */
  const [exportAllLabels, setExportAllLabels] = useState(false);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = requestRef.current + 1;
    requestRef.current = requestId;
    const updatedAfter = recent
      ? new Date(Date.now() - Number(recent) * 24 * 60 * 60 * 1000).toISOString()
      : null;
    const next = await graphService
      .get({
        categoryId: categoryId || null,
        tagId: tagId || null,
        includeOrphans,
        updatedAfter,
        focusNoteId: focusId,
        hops,
      })
      .catch((cause: unknown) => {
        if (requestId === requestRef.current) setError(errorMessage(cause));
        return null;
      });
    if (requestId !== requestRef.current || !next) return;
    setData(next);
    setError(null);
    if (selectedId && !next.nodes.some((node) => node.id === selectedId)) setSelectedId(null);
  }, [categoryId, tagId, includeOrphans, recent, focusId, hops, selectedId]);

  useEffect(() => {
    void load();
    // 切换 / 打开 / 新建知识库后自动重新查询，避免残留上一个库的图谱
  }, [load, workspaceId]);

  const communities = useMemo(
    () => detectCommunities((data?.nodes ?? []).map((node) => node.id), data?.edges ?? [], { resolution }),
    [data, resolution],
  );
  const nodeColors = useMemo(() => {
    const colors = new Map<string, string>();
    (data?.nodes ?? []).forEach((node) => {
      colors.set(
        node.id,
        colorMode === 'community'
          ? communityColor(communities.get(node.id))
          : categoryColor(node.categoryId),
      );
    });
    return colors;
  }, [data, colorMode, communities]);

  const legend = useMemo(() => {
    if (colorMode === 'community' && data) {
      return communitySizes(communities)
        .slice(0, 6)
        .map((item) => ({
          id: `community-${item.id}`,
          count: item.count,
          color: communityColor(item.id),
          label: `社区 ${item.id + 1}`,
        }));
    }

    if (!data) return [];
    const counts = new Map<string, number>();
    data.nodes.forEach((node) => {
      const key = node.categoryId ?? '';
      counts.set(key, (counts.get(key) ?? 0) + 1);
    });
    return [...counts.entries()]
      .sort((left, right) => right[1] - left[1])
      .slice(0, 6)
      .map(([id, count]) => ({
        id,
        count,
        color: categoryColor(id || null),
        label: id ? (pathOf(id) || '未命名分类') : '未分类',
      }));
  }, [data, pathOf, colorMode, communities]);

  const stamp = () => new Date().toISOString().slice(0, 16).replace(/[-:T]/g, '');

  /** 导出「带图例的 PNG」：落到知识库 attachments/，随工作区一起备份 */
  const exportPng = async (dataUrl: string) => {
    try {
      const saved = await attachmentService.save({
        fileName: `知识图谱-${stamp()}.png`,
        mimeType: 'image/png',
        dataBase64: dataUrl,
      });
      toast.success(`已导出到 ${saved.relativePath}`);
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  /** 导出 SVG：矢量、可再编辑，落盘位置同上 */
  const exportSvg = async (svg: string) => {
    try {
      const saved = await attachmentService.save({
        fileName: `知识图谱-${stamp()}.svg`,
        mimeType: 'image/svg+xml',
        dataBase64: toBase64Utf8(svg),
      });
      toast.success(`已导出到 ${saved.relativePath}`);
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const selectedNode = data?.nodes.find((node) => node.id === selectedId) ?? null;
  const orphanCount = data?.nodes.filter((node) => node.orphan).length ?? 0;

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <header className="flex flex-wrap items-center gap-2 border-b border-[var(--border-subtle)] px-4 py-2.5">
        <Network className="h-4 w-4 text-[var(--brand)]" />
        <h1 className="text-sm font-semibold text-[var(--ink)]">知识图谱</h1>
        {data ? (
          <span className="text-[11px] text-[var(--ink-subtle)]">
            {formatCount(data.nodes.length)} 个节点 · {formatCount(data.edges.length)} 条连接 ·{' '}
            {data.durationMs} ms
          </span>
        ) : null}
        <div className="ml-auto flex items-center gap-1.5">
          <select
            aria-label="按分类筛选图谱"
            value={categoryId}
            onChange={(event) => setCategoryId(event.target.value)}
            className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
          >
            <option value="">全部分类</option>
            {categories.map((category) => (
              <option key={category.id} value={category.id}>
                {pathOf(category.id)}
              </option>
            ))}
          </select>
          <select
            aria-label="按标签筛选图谱"
            value={tagId}
            onChange={(event) => setTagId(event.target.value)}
            className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
          >
            <option value="">全部标签</option>
            {tags.map((tag) => (
              <option key={tag.id} value={tag.id}>
                {tag.name}
              </option>
            ))}
          </select>
          <label className="flex items-center gap-1 text-xs text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={includeOrphans}
              onChange={(event) => setIncludeOrphans(event.target.checked)}
            />
            孤立节点
          </label>
          <select
            aria-label="按更新时间筛选图谱"
            value={recent}
            onChange={(event) => setRecent(event.target.value)}
            className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
          >
            <option value="">全部时间</option>
            <option value="7">最近 7 天</option>
            <option value="30">最近 30 天</option>
            <option value="90">最近 90 天</option>
          </select>
          <select
            aria-label="图谱着色方式"
            value={colorMode}
            onChange={(event) => setColorMode(event.target.value as 'category' | 'community')}
            className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
          >
            <option value="category">按分类着色</option>
            <option value="community">按社区着色</option>
          </select>
          {colorMode === 'community' ? (
            <select
              aria-label="社区粒度"
              value={resolution}
              onChange={(event) => setResolution(Number(event.target.value))}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
              title="粒度越大社区越细"
            >
              <option value={0.5}>粒度：粗</option>
              <option value={1}>粒度：标准</option>
              <option value={1.5}>粒度：细</option>
              <option value={2}>粒度：很细</option>
            </select>
          ) : null}
          <label className="flex items-center gap-1 text-xs text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={exportAllLabels}
              onChange={(event) => setExportAllLabels(event.target.checked)}
            />
            导出含标题
          </label>
          <Button
            size="sm"
            variant={focusId ? 'default' : 'secondary'}
            disabled={!currentNoteId}
            title={currentNoteId ? '只看当前笔记及其邻居' : '先在左侧选一篇笔记'}
            onClick={() => {
              if (!currentNoteId) return;
              setFocusId((value) => (value ? null : currentNoteId));
            }}
          >
            <Crosshair className="h-3.5 w-3.5" />
            {focusId ? '取消聚焦' : '聚焦当前笔记'}
          </Button>
          {focusId ? (
            <select
              aria-label="聚焦跳数"
              value={hops}
              onChange={(event) => setHops(Number(event.target.value))}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-1.5 text-xs text-[var(--ink)]"
            >
              <option value={1}>1 跳</option>
              <option value={2}>2 跳</option>
            </select>
          ) : null}
          <Button size="icon" variant="ghost" title="重新查询" onClick={() => void load()}>
            <RefreshCw className={cn('h-4 w-4', !data && 'animate-spin')} />
          </Button>
        </div>
      </header>

      {data?.truncated ? (
        <div className="border-b border-amber-500/30 bg-amber-500/10 px-4 py-1.5 text-[11px] text-amber-700 dark:text-amber-400">
          节点或连接过多（共 {formatCount(data.totalNodes)} 个节点 / {formatCount(data.totalEdges)}{' '}
          条连接），当前显示 {formatCount(data.nodes.length)} 个节点：
          已按**分类配额 + 连接优先**抽样——每个分类都保留代表，并把高连接节点的邻居一起带进来，
          因此看到的是成片的簇而不是散点。想看得更细可按分类 / 标签筛选，或聚焦到某篇笔记。
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1">
        <div className="min-w-0 flex-1">
          {data === null ? (
            error ? (
              <EmptyState
                icon={Network}
                title="图谱加载失败"
                description={error}
                action={
                  <Button variant="secondary" onClick={() => void load()}>
                    重试
                  </Button>
                }
                className="h-full"
              />
            ) : (
              <div className="space-y-2 p-4">
                <Skeleton className="h-64 w-full" />
                <Skeleton className="h-4 w-1/3" />
              </div>
            )
          ) : data.nodes.length === 0 ? (
            <EmptyState
              icon={Unlink}
              title="当前筛选下没有节点"
              description="试试放宽筛选条件，或先写几篇带 [[双向链接]] 的笔记。"
              className="h-full"
            />
          ) : (
            <ForceGraph
              nodes={data.nodes}
              edges={data.edges}
              selectedId={selectedId}
              colors={nodeColors}
              onSelect={setSelectedId}
              onOpen={(id) => navigate(ROUTES.note(id))}
              legend={legend}
              onExportPng={(dataUrl) => void exportPng(dataUrl)}
              onExportFailed={(reason) => toast.error(reason)}
              onExportSvg={(svg) => void exportSvg(svg)}
              exportAllLabels={exportAllLabels}
            />
          )}
        </div>

        <aside className="hidden w-56 shrink-0 flex-col gap-3 border-l border-[var(--border-subtle)] p-3 lg:flex">
          <section className="space-y-1.5">
            <h2 className="text-xs font-medium text-[var(--ink)]">图例</h2>
            {legend.length === 0 ? (
              <p className="text-[11px] text-[var(--ink-subtle)]">暂无节点</p>
            ) : (
              <ul className="space-y-1">
                {legend.map((item) => (
                  <li key={item.id || 'inbox'} className="flex items-center gap-2 text-[11px]">
                    <span
                      className="h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ backgroundColor: item.color }}
                    />
                    <span className="min-w-0 flex-1 truncate text-[var(--ink-muted)]">
                      {item.label}
                    </span>
                    <span className="tabular-nums text-[var(--ink-subtle)]">{item.count}</span>
                  </li>
                ))}
              </ul>
            )}
            <p className="text-[11px] text-[var(--ink-subtle)]">
              节点大小 = 连接数；未分类节点为灰色；当前有 {orphanCount} 个孤立节点。
            </p>
          </section>

          <section className="space-y-1.5 border-t border-[var(--border-subtle)] pt-3">
            <h2 className="text-xs font-medium text-[var(--ink)]">选中的笔记</h2>
            {selectedNode ? (
              <>
                <p className="text-xs text-[var(--ink)]">{selectedNode.title}</p>
                <p className="text-[11px] text-[var(--ink-subtle)]">
                  {selectedNode.degree} 个邻居
                  {selectedNode.tags.length > 0 ? ` · ${selectedNode.tags.join('、')}` : ''}
                </p>
                <div className="flex gap-1.5">
                  <Button size="sm" variant="secondary" onClick={() => setFocusId(selectedNode.id)}>
                    <Crosshair className="h-3 w-3" /> 聚焦
                  </Button>
                  <Button size="sm" onClick={() => navigate(ROUTES.note(selectedNode.id))}>
                    <ExternalLink className="h-3 w-3" /> 打开
                  </Button>
                </div>
              </>
            ) : (
              <p className="text-[11px] text-[var(--ink-subtle)]">
                单击节点查看详情，双击直接打开笔记。
              </p>
            )}
          </section>

          <section className="mt-auto space-y-1 border-t border-[var(--border-subtle)] pt-3">
            <Button
              size="sm"
              variant="ghost"
              className="w-full justify-start"
              onClick={() => toast.info('用滚轮缩放、拖拽平移，按 F 可让整张图适应窗口')}
            >
              <Maximize2 className="h-3.5 w-3.5" />
              操作说明
            </Button>
          </section>
        </aside>
      </div>
    </div>
  );
}

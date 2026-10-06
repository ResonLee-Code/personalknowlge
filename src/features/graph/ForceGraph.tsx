import { useCallback, useEffect, useRef, useState } from 'react';

import { buildGraphSvg, drawLegendOnCanvas, type ExportLegendItem } from '@/lib/graphExport';
import { categoryColor } from '@/lib/graphColors';
import { createLayout, layoutBounds, reheat, stepLayout, type LayoutState } from '@/lib/graphLayout';
import type { GraphEdge, GraphNode } from '@/types';

interface ForceGraphProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selectedId?: string | null;
  /** 单击节点：设为聚焦 */
  onSelect?: (id: string) => void;
  /** 双击节点：打开笔记 */
  onOpen?: (id: string) => void;
  /** 布局就绪回调（用于「适应窗口」等外部操作） */
  onSettled?: () => void;
  /** 节点配色（按 id 覆盖默认的分类配色；社区着色模式使用） */
  colors?: Map<string, string>;
  /** 导出「带图例的 PNG」（base64 data URL，由页面落盘） */
  onExportPng?: (dataUrl: string) => void;
  /** 导出 SVG（文本，由页面落盘） */
  onExportSvg?: (svg: string) => void;
  /** 导出时写入的图例 */
  legend?: ExportLegendItem[];
  /** 导出时是否标注全部节点标题（默认只标枢纽） */
  exportAllLabels?: boolean;
}

/**
 * Canvas 力导向图（V1.1 M3）。
 *
 * - 拖拽节点会固定该节点并重新加热布局；拖空白区域平移画布；滚轮缩放（以指针为中心）；
 * - 收敛后停止渲染帧，交互时再加热，保证 3000 节点以下也能保持流畅；
 * - 标签按缩放级别显示：放大后显示全部标题，否则只显示度数较高的节点。
 */
export function ForceGraph({
  nodes,
  edges,
  selectedId,
  onSelect,
  onOpen,
  onSettled,
  colors,
  onExportPng,
  onExportSvg,
  legend = [],
  exportAllLabels = false,
}: ForceGraphProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const stateRef = useRef<LayoutState | null>(null);
  const frameRef = useRef<number | null>(null);
  const viewRef = useRef({ scale: 1, offsetX: 0, offsetY: 0 });
  const dragRef = useRef<{
    mode: 'node' | 'pan';
    nodeId?: string;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
    moved: boolean;
  } | null>(null);
  const hoverRef = useRef<string | null>(null);
  /** 布局收敛后自动适应窗口（fit 在渲染后定义，用 ref 传递） */
  const fitRef = useRef<() => void>(() => {});
  const [hovered, setHovered] = useState<{ title: string; x: number; y: number } | null>(null);
  const [ready, setReady] = useState(false);

  /** 屏幕坐标 → 画布坐标 */
  const toCanvas = useCallback((clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    const view = viewRef.current;
    return {
      x: (clientX - rect.left - view.offsetX) / view.scale,
      y: (clientY - rect.top - view.offsetY) / view.scale,
    };
  }, []);

  const nodeAt = useCallback(
    (x: number, y: number): string | null => {
      const state = stateRef.current;
      if (!state) return null;
      for (let index = state.nodes.length - 1; index >= 0; index -= 1) {
        const node = state.nodes[index]!;
        const radius = node.radius + 4;
        if (Math.hypot(node.x - x, node.y - y) <= radius) return node.id;
      }
      return null;
    },
    [],
  );

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    const state = stateRef.current;
    if (!canvas || !state) return;
    const context = canvas.getContext('2d');
    if (!context) return;

    const ratio = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    if (canvas.width !== Math.round(width * ratio) || canvas.height !== Math.round(height * ratio)) {
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
    }

    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);
    const view = viewRef.current;
    context.save();
    context.translate(view.offsetX, view.offsetY);
    context.scale(view.scale, view.scale);

    // 边
    context.lineWidth = 1 / view.scale;
    for (const edge of state.edges) {
      const sourceIndex = state.index.get(edge.source);
      const targetIndex = state.index.get(edge.target);
      if (sourceIndex === undefined || targetIndex === undefined) continue;
      const source = state.nodes[sourceIndex]!;
      const target = state.nodes[targetIndex]!;
      const highlighted =
        selectedId !== undefined &&
        selectedId !== null &&
        (edge.source === selectedId || edge.target === selectedId);
      context.strokeStyle = highlighted
        ? 'rgba(99, 102, 241, 0.75)'
        : `rgba(148, 163, 184, ${Math.min(0.5, 0.18 + edge.weight * 0.08)})`;
      context.beginPath();
      context.moveTo(source.x, source.y);
      context.lineTo(target.x, target.y);
      context.stroke();
    }

    // 节点
    for (const node of state.nodes) {
      const selected = node.id === selectedId;
      context.beginPath();
      context.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
      context.fillStyle = colors?.get(node.id) ?? categoryColor(node.categoryId);
      context.globalAlpha = node.orphan ? 0.55 : 1;
      context.fill();
      context.globalAlpha = 1;
      if (selected || hoverRef.current === node.id) {
        context.lineWidth = 2 / view.scale;
        context.strokeStyle = selected ? '#6366f1' : 'rgba(15, 23, 42, 0.55)';
        context.stroke();
      }
    }

    // 标签：缩放足够大时全显示，否则只显示枢纽与选中/悬浮节点
    const showAll = view.scale > 0.85;
    context.font = `${11 / view.scale}px ui-sans-serif, system-ui, sans-serif`;
    context.fillStyle = 'rgba(100, 116, 139, 1)';
    context.textAlign = 'center';
    context.textBaseline = 'top';
    for (const node of state.nodes) {
      const important =
        showAll || node.degree >= 3 || node.id === selectedId || node.id === hoverRef.current;
      if (!important) continue;
      const title = nodes.find((item) => item.id === node.id)?.title ?? node.id;
      const text = title.length > 14 ? `${title.slice(0, 13)}…` : title;
      context.fillText(text, node.x, node.y + node.radius + 2);
    }

    context.restore();
  }, [nodes, selectedId, colors]);

  /** 重建布局（数据变化时） */
  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const width = wrap.clientWidth || 800;
    const height = wrap.clientHeight || 600;
    stateRef.current = createLayout(nodes, edges, { width, height });
    viewRef.current = { scale: 1, offsetX: 0, offsetY: 0 };
    setReady(false);
    reheat(stateRef.current, 1);

    // 每帧推进多步：布局更快收敛（≈1s 内稳定），同时保持动画顺滑
    const STEPS_PER_FRAME = 2;
    let frames = 0;
    const tick = () => {
      const state = stateRef.current;
      if (!state) return;
      let moving = true;
      for (let step = 0; step < STEPS_PER_FRAME && moving; step += 1) {
        moving = stepLayout(state);
      }
      frames += 1;
      // 收敛过程中持续取景，避免节点跑到视野之外
      if (frames % 15 === 0) fitRef.current();
      draw();
      if (moving) {
        frameRef.current = requestAnimationFrame(tick);
      } else {
        frameRef.current = null;
        fitRef.current();
        setReady(true);
        onSettled?.();
      }
    };
    frameRef.current = requestAnimationFrame(tick);

    return () => {
      if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    };
    // onSettled 只用于通知，不参与重建
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, edges, draw]);

  /** 尺寸变化：按新尺寸重建（保持节点位置不变则无法直接复用，简单起见重置） */
  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap || typeof ResizeObserver === 'undefined') return;
    let timer: number | null = null;
    const observer = new ResizeObserver(() => {
      if (timer !== null) window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        const state = stateRef.current;
        if (!state) return;
        state.width = wrap.clientWidth || state.width;
        state.height = wrap.clientHeight || state.height;
        reheat(state, 0.4);
        const tick = () => {
          const current = stateRef.current;
          if (!current) return;
          const moving = stepLayout(current);
          draw();
          if (moving) frameRef.current = requestAnimationFrame(tick);
          else frameRef.current = null;
        };
        if (frameRef.current === null) frameRef.current = requestAnimationFrame(tick);
      }, 120);
    });
    observer.observe(wrap);
    return () => {
      observer.disconnect();
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [draw]);

  /** 适应窗口 */
  const fit = useCallback(() => {
    const canvas = canvasRef.current;
    const state = stateRef.current;
    if (!canvas || !state || state.nodes.length === 0) return;
    const bounds = layoutBounds(state);
    const padding = 60;
    const contentWidth = Math.max(1, bounds.maxX - bounds.minX);
    const contentHeight = Math.max(1, bounds.maxY - bounds.minY);
    const scale = Math.min(
      1.6,
      Math.max(0.2, Math.min((canvas.clientWidth - padding) / contentWidth, (canvas.clientHeight - padding) / contentHeight)),
    );
    viewRef.current = {
      scale,
      offsetX: canvas.clientWidth / 2 - ((bounds.minX + bounds.maxX) / 2) * scale,
      offsetY: canvas.clientHeight / 2 - ((bounds.minY + bounds.maxY) / 2) * scale,
    };
    draw();
  }, [draw]);

  useEffect(() => {
    fitRef.current = fit;
  }, [fit]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === 'f' && !event.metaKey && !event.ctrlKey) fit();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [fit]);

  const startFrame = useCallback(() => {
    if (frameRef.current !== null) return;
    const tick = () => {
      const state = stateRef.current;
      if (!state) return;
      const moving = stepLayout(state);
      draw();
      frameRef.current = moving ? requestAnimationFrame(tick) : null;
    };
    frameRef.current = requestAnimationFrame(tick);
  }, [draw]);

  return (
    <div ref={wrapRef} className="relative h-full w-full overflow-hidden">
      <canvas
        ref={canvasRef}
        data-testid="graph-canvas"
        className="h-full w-full cursor-grab touch-none active:cursor-grabbing"
        onWheel={(event) => {
          const canvas = canvasRef.current;
          if (!canvas) return;
          const rect = canvas.getBoundingClientRect();
          const pointerX = event.clientX - rect.left;
          const pointerY = event.clientY - rect.top;
          const view = viewRef.current;
          const factor = event.deltaY < 0 ? 1.12 : 1 / 1.12;
          const nextScale = Math.min(4, Math.max(0.15, view.scale * factor));
          const ratio = nextScale / view.scale;
          view.offsetX = pointerX - (pointerX - view.offsetX) * ratio;
          view.offsetY = pointerY - (pointerY - view.offsetY) * ratio;
          view.scale = nextScale;
          draw();
        }}
        onPointerDown={(event) => {
          const point = toCanvas(event.clientX, event.clientY);
          const hit = nodeAt(point.x, point.y);
          dragRef.current = {
            mode: hit ? 'node' : 'pan',
            nodeId: hit ?? undefined,
            startX: event.clientX,
            startY: event.clientY,
            originX: hit ? point.x : viewRef.current.offsetX,
            originY: hit ? point.y : viewRef.current.offsetY,
            moved: false,
          };
          if (hit) {
            stateRef.current?.fixed.add(hit);
            (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
          }
        }}
        onPointerMove={(event) => {
          const drag = dragRef.current;
          const state = stateRef.current;
          if (!drag || !state) {
            const point = toCanvas(event.clientX, event.clientY);
            const hit = nodeAt(point.x, point.y);
            if (hit !== hoverRef.current) {
              hoverRef.current = hit;
              const node = state?.nodes.find((item) => item.id === hit);
              const meta = nodes.find((item) => item.id === hit);
              setHovered(
                hit && node && meta
                  ? {
                      title: meta.title,
                      x: node.x * viewRef.current.scale + viewRef.current.offsetX,
                      y: node.y * viewRef.current.scale + viewRef.current.offsetY,
                    }
                  : null,
              );
              draw();
            }
            return;
          }

          if (Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) > 3) {
            drag.moved = true;
          }
          if (drag.mode === 'pan') {
            viewRef.current.offsetX = drag.originX + (event.clientX - drag.startX);
            viewRef.current.offsetY = drag.originY + (event.clientY - drag.startY);
            draw();
            return;
          }
          const node = state.nodes.find((item) => item.id === drag.nodeId);
          if (!node) return;
          const point = toCanvas(event.clientX, event.clientY);
          node.x = point.x;
          node.y = point.y;
          node.vx = 0;
          node.vy = 0;
          reheat(state, 0.5);
          startFrame();
        }}
        onPointerUp={(event) => {
          const drag = dragRef.current;
          dragRef.current = null;
          if (drag?.mode === 'node' && drag.nodeId) {
            stateRef.current?.fixed.delete(drag.nodeId);
            if (!drag.moved) onSelect?.(drag.nodeId);
          }
          (event.target as HTMLElement).releasePointerCapture?.(event.pointerId);
        }}
        onDoubleClick={(event) => {
          const point = toCanvas(event.clientX, event.clientY);
          const hit = nodeAt(point.x, point.y);
          if (hit) onOpen?.(hit);
        }}
      />
      {hovered ? (
        <div
          className="pointer-events-none absolute z-10 max-w-[240px] -translate-x-1/2 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-raised)] px-2 py-1 text-[11px] text-[var(--ink)] shadow-lg"
          style={{ left: hovered.x, top: hovered.y + 12 }}
        >
          {hovered.title}
        </div>
      ) : null}
      {onExportPng || onExportSvg ? (
        <div className="absolute right-2 top-2 flex gap-1">
          {onExportPng ? (
            <button
              type="button"
              className="rounded-md border border-[var(--border-subtle)] bg-[var(--surface-raised)] px-2 py-1 text-[11px] text-[var(--ink-muted)] transition-colors hover:text-[var(--ink)]"
              onClick={() => {
                const canvas = canvasRef.current;
                const state = stateRef.current;
                if (!canvas || !state) return;
                try {
                  // 复制当前画面，再补上图例与标题，保证分享出去能自解释
                  const output = document.createElement('canvas');
                  output.width = canvas.width;
                  output.height = canvas.height;
                  const context = output.getContext('2d');
                  if (!context) return;
                  context.fillStyle = '#ffffff';
                  context.fillRect(0, 0, output.width, output.height);
                  context.drawImage(canvas, 0, 0);
                  const ratio = output.width / (canvas.clientWidth || output.width);
                  if (exportAllLabels) {
                    // 导出时才补全标题：屏幕上为保持清爽只画枢纽，导出件需要能独立读懂
                    context.save();
                    context.scale(ratio, ratio);
                    context.translate(viewRef.current.offsetX, viewRef.current.offsetY);
                    context.scale(viewRef.current.scale, viewRef.current.scale);
                    context.font = `${11 / viewRef.current.scale}px ui-sans-serif, system-ui, sans-serif`;
                    context.fillStyle = 'rgba(71, 85, 105, 1)';
                    context.textAlign = 'center';
                    context.textBaseline = 'top';
                    for (const node of state.nodes) {
                      const title = nodes.find((item) => item.id === node.id)?.title ?? node.id;
                      const text = title.length > 16 ? `${title.slice(0, 15)}…` : title;
                      context.fillText(text, node.x, node.y + node.radius + 2);
                    }
                    context.restore();
                  }
                  context.save();
                  context.scale(ratio, ratio);
                  drawLegendOnCanvas(
                    context,
                    legend,
                    { width: canvas.clientWidth, height: canvas.clientHeight },
                    '知识图谱',
                  );
                  context.restore();
                  onExportPng(output.toDataURL('image/png'));
                } catch {
                  // 无头环境没有 Canvas 实现，忽略
                }
              }}
            >
              导出 PNG
            </button>
          ) : null}
          {onExportSvg ? (
            <button
              type="button"
              className="rounded-md border border-[var(--border-subtle)] bg-[var(--surface-raised)] px-2 py-1 text-[11px] text-[var(--ink-muted)] transition-colors hover:text-[var(--ink)]"
              onClick={() => {
                const canvas = canvasRef.current;
                const state = stateRef.current;
                if (!canvas || !state) return;
                onExportSvg(
                  buildGraphSvg({
                    width: canvas.clientWidth || 800,
                    height: canvas.clientHeight || 600,
                    nodes: state.nodes.map((node) => ({
                      id: node.id,
                      x: node.x,
                      y: node.y,
                      radius: node.radius,
                      color: colors?.get(node.id) ?? categoryColor(node.categoryId),
                      label:
                        exportAllLabels || node.degree >= 3 || state.nodes.length <= 30
                          ? nodes.find((item) => item.id === node.id)?.title
                          : undefined,
                    })),
                    edges: state.edges.flatMap((edge) => {
                      const sourceIndex = state.index.get(edge.source);
                      const targetIndex = state.index.get(edge.target);
                      if (sourceIndex === undefined || targetIndex === undefined) return [];
                      const source = state.nodes[sourceIndex]!;
                      const target = state.nodes[targetIndex]!;
                      return [
                        {
                          x1: source.x,
                          y1: source.y,
                          x2: target.x,
                          y2: target.y,
                          weight: edge.weight,
                          highlighted: selectedId
                            ? edge.source === selectedId || edge.target === selectedId
                            : false,
                        },
                      ];
                    }),
                    legend,
                    title: '知识图谱',
                    subtitle: `导出时间 ${new Date().toLocaleString('zh-CN')}`,
                  }),
                );
              }}
            >
              导出 SVG
            </button>
          ) : null}
        </div>
      ) : null}
      <div className="pointer-events-none absolute bottom-2 left-2 text-[11px] text-[var(--ink-subtle)]">
        {ready ? '拖拽节点可固定位置 · 滚轮缩放 · 双击打开笔记 · F 适应窗口' : '正在计算布局…'}
      </div>
    </div>
  );
}

/**
 * 图谱导出（V1.1.2）：把当前布局渲染成可分享的 SVG 或「带图例的 PNG」。
 *
 * 这里是纯函数，便于单测；Canvas 侧只负责取像素与落盘。
 */

export interface ExportNode {
  id: string;
  x: number;
  y: number;
  radius: number;
  color: string;
  label?: string;
}

export interface ExportEdge {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  weight: number;
  highlighted?: boolean;
}

export interface ExportLegendItem {
  color: string;
  label: string;
  count: number;
}

export interface GraphExportInput {
  width: number;
  height: number;
  nodes: ExportNode[];
  edges: ExportEdge[];
  legend: ExportLegendItem[];
  /** 标题与副标题（导出时间等信息） */
  title?: string;
  subtitle?: string;
  /** 画布背景（默认浅色，保证分享出去可读） */
  background?: string;
}

/** XML 转义：节点标题可能包含 `<`、`&` 等字符。 */
export function escapeXml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/** UTF-8 文本 → Base64（SVG 里含中文，不能直接用 btoa）。 */
export function toBase64Utf8(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary);
}

/** 生成自包含 SVG（含图例），可直接被浏览器/设计工具打开。 */
export function buildGraphSvg(input: GraphExportInput): string {
  const background = input.background ?? '#ffffff';
  const parts: string[] = [];
  parts.push(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${input.width}" height="${input.height}" viewBox="0 0 ${input.width} ${input.height}">`,
  );
  parts.push(`<rect width="100%" height="100%" fill="${background}"/>`);

  if (input.title) {
    parts.push(
      `<text x="20" y="30" font-family="system-ui, sans-serif" font-size="16" font-weight="600" fill="#0f172a">${escapeXml(
        input.title,
      )}</text>`,
    );
  }
  if (input.subtitle) {
    parts.push(
      `<text x="20" y="50" font-family="system-ui, sans-serif" font-size="11" fill="#64748b">${escapeXml(
        input.subtitle,
      )}</text>`,
    );
  }

  for (const edge of input.edges) {
    const alpha = Math.min(0.6, 0.2 + edge.weight * 0.08);
    parts.push(
      `<line x1="${edge.x1.toFixed(2)}" y1="${edge.y1.toFixed(2)}" x2="${edge.x2.toFixed(
        2,
      )}" y2="${edge.y2.toFixed(2)}" stroke="${
        edge.highlighted ? '#6366f1' : '#94a3b8'
      }" stroke-width="${edge.highlighted ? 2 : 1}" stroke-opacity="${edge.highlighted ? 0.9 : alpha.toFixed(2)}"/>`,
    );
  }

  for (const node of input.nodes) {
    parts.push(
      `<circle cx="${node.x.toFixed(2)}" cy="${node.y.toFixed(2)}" r="${node.radius.toFixed(
        2,
      )}" fill="${node.color}"/>`,
    );
  }
  for (const node of input.nodes) {
    if (!node.label) continue;
    parts.push(
      `<text x="${node.x.toFixed(2)}" y="${(node.y + node.radius + 12).toFixed(
        2,
      )}" text-anchor="middle" font-family="system-ui, sans-serif" font-size="11" fill="#475569">${escapeXml(
        node.label,
      )}</text>`,
    );
  }

  // 图例：右下角半透明面板
  if (input.legend.length > 0) {
    const rowHeight = 20;
    const panelWidth = 190;
    const panelHeight = 34 + input.legend.length * rowHeight;
    const x = input.width - panelWidth - 20;
    const y = input.height - panelHeight - 20;
    parts.push(
      `<rect x="${x}" y="${y}" width="${panelWidth}" height="${panelHeight}" rx="8" fill="#ffffff" fill-opacity="0.92" stroke="#e2e8f0"/>`,
    );
    parts.push(
      `<text x="${x + 12}" y="${y + 22}" font-family="system-ui, sans-serif" font-size="12" font-weight="600" fill="#0f172a">图例</text>`,
    );
    input.legend.forEach((item, index) => {
      const rowY = y + 34 + index * rowHeight + 10;
      parts.push(
        `<circle cx="${x + 18}" cy="${rowY - 4}" r="5" fill="${item.color}"/>`,
      );
      parts.push(
        `<text x="${x + 32}" y="${rowY}" font-family="system-ui, sans-serif" font-size="11" fill="#475569">${escapeXml(
          `${item.label} (${item.count})`,
        )}</text>`,
      );
    });
  }

  parts.push('</svg>');
  return parts.join('\n');
}

/** 图例面板的几何参数（与 Canvas 绘制共用，便于单测）。 */
export interface LegendPanelOptions {
  rowHeight?: number;
  panelWidth?: number;
  margin?: number;
  headerHeight?: number;
}

/** 图例面板的位置与尺寸（右下角锚定）；没有图例时返回 `null`。 */
export function legendPanelRect(
  legendCount: number,
  size: { width: number; height: number },
  options: LegendPanelOptions = {},
): { x: number; y: number; width: number; height: number } | null {
  if (legendCount <= 0) return null;
  const rowHeight = options.rowHeight ?? 20;
  const panelWidth = options.panelWidth ?? 190;
  const margin = options.margin ?? 20;
  const headerHeight = options.headerHeight ?? 34;
  const panelHeight = headerHeight + legendCount * rowHeight;
  return {
    x: size.width - panelWidth - margin,
    y: size.height - panelHeight - margin,
    width: panelWidth,
    height: panelHeight,
  };
}

/** 在 Canvas 上绘制与 SVG 一致的图例面板（用于「带图例的 PNG」）。 */
export function drawLegendOnCanvas(
  context: CanvasRenderingContext2D,
  legend: ExportLegendItem[],
  size: { width: number; height: number },
  title?: string,
): void {
  if (title) {
    context.font = '600 16px system-ui, sans-serif';
    context.fillStyle = '#0f172a';
    context.textAlign = 'left';
    context.textBaseline = 'alphabetic';
    context.fillText(title, 20, 30);
  }
  const rect = legendPanelRect(legend.length, size);
  if (!rect) return;
  const rowHeight = 20;
  const { x, y } = rect;

  context.save();
  context.fillStyle = 'rgba(255, 255, 255, 0.92)';
  context.strokeStyle = '#e2e8f0';
  context.lineWidth = 1;
  context.beginPath();
  context.roundRect(x, y, rect.width, rect.height, 8);
  context.fill();
  context.stroke();

  context.fillStyle = '#0f172a';
  context.font = '600 12px system-ui, sans-serif';
  context.fillText('图例', x + 12, y + 22);

  legend.forEach((item, index) => {
    const rowY = y + 34 + index * rowHeight + 10;
    context.beginPath();
    context.arc(x + 18, rowY - 4, 5, 0, Math.PI * 2);
    context.fillStyle = item.color;
    context.fill();
    context.fillStyle = '#475569';
    context.font = '11px system-ui, sans-serif';
    context.fillText(`${item.label} (${item.count})`, x + 32, rowY);
  });
  context.restore();
}

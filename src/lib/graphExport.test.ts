import { describe, expect, it } from 'vitest';

import {
  buildGraphSvg,
  escapeXml,
  legendPanelRect,
  toBase64Utf8,
} from '@/lib/graphExport';

const base = {
  width: 400,
  height: 300,
  nodes: [{ id: 'a', x: 10, y: 20, radius: 4, color: '#6366f1', label: '索引<设计>' }],
  edges: [{ x1: 10, y1: 20, x2: 100, y2: 80, weight: 2, highlighted: true }],
  legend: [{ color: '#10b981', label: '技术', count: 3 }],
  title: '知识图谱',
  subtitle: '2026-10-07',
};

describe('buildGraphSvg', () => {
  it('输出自包含 SVG，包含节点、边、图例与标题', () => {
    const svg = buildGraphSvg(base);

    expect(svg.startsWith('<svg xmlns=')).toBe(true);
    expect(svg).toContain('<circle cx="10.00" cy="20.00" r="4.00" fill="#6366f1"/>');
    expect(svg).toContain('stroke="#6366f1"');
    expect(svg).toContain('知识图谱');
    expect(svg).toContain('技术 (3)');
    expect(svg.trimEnd().endsWith('</svg>')).toBe(true);
  });

  it('转义标题里的 XML 特殊字符', () => {
    const svg = buildGraphSvg(base);
    expect(svg).toContain('索引&lt;设计&gt;');
    expect(svg).not.toContain('索引<设计>');
  });

  it('没有图例时不画图例面板', () => {
    const svg = buildGraphSvg({ ...base, legend: [] });
    expect(svg).not.toContain('图例');
    expect(svg).not.toContain('<rect x="190"');
  });

  it('空图也能生成合法 SVG', () => {
    const svg = buildGraphSvg({ width: 100, height: 100, nodes: [], edges: [], legend: [] });
    expect(svg).toContain('<svg');
    expect(svg).toContain('</svg>');
  });
});

describe('escapeXml / toBase64Utf8', () => {
  it('转义全部保留字符', () => {
    expect(escapeXml(`<&>"'`)).toBe('&lt;&amp;&gt;&quot;&apos;');
  });

  it('中文可以正确编码为 Base64 并可解回', () => {
    const encoded = toBase64Utf8('知识图谱 A');
    const decoded = Buffer.from(encoded, 'base64').toString('utf8');
    expect(decoded).toBe('知识图谱 A');
  });
});

describe('legendPanelRect', () => {
  const size = { width: 800, height: 600 };

  it('没有图例时不画面板', () => {
    expect(legendPanelRect(0, size)).toBeNull();
  });

  it('高度随条目数增长，且锚定在右下角、留出边距', () => {
    const one = legendPanelRect(1, size)!;
    const five = legendPanelRect(5, size)!;
    expect(five.height - one.height).toBe(4 * 20);
    expect(one.x + one.width).toBe(size.width - 20);
    expect(one.y + one.height).toBe(size.height - 20);
  });

  it('面板始终落在画布范围内', () => {
    const rect = legendPanelRect(3, { width: 400, height: 300 })!;
    expect(rect.x).toBeGreaterThanOrEqual(0);
    expect(rect.y).toBeGreaterThanOrEqual(0);
    expect(rect.x + rect.width).toBeLessThanOrEqual(400);
    expect(rect.y + rect.height).toBeLessThanOrEqual(300);
  });
});

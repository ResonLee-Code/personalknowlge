import type { GraphData, GraphInsights, GraphInsightsQuery, GraphQuery } from '@/types';

import { call } from './api';

/** 知识图谱（V1.1 M3）：节点来自笔记，边来自已解析的双向链接。 */
export const graphService = {
  get: (query: GraphQuery = {}) => call<GraphData>('graph_get', { query }),
  /** Graph 2.0：可解释近邻、近重复、孤儿、桥梁与孤岛分类。 */
  insights: (query: GraphInsightsQuery = {}) => call<GraphInsights>('graph_insights', { query }),
};

export type { GraphData, GraphInsights, GraphInsightsQuery, GraphQuery };

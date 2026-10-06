import type { GraphData, GraphQuery } from '@/types';

import { call } from './api';

/** 知识图谱（V1.1 M3）：节点来自笔记，边来自已解析的双向链接。 */
export const graphService = {
  get: (query: GraphQuery = {}) => call<GraphData>('graph_get', { query }),
};

export type { GraphData, GraphQuery };

import type {
  RenderedTemplate,
  RenderTemplateInput,
  SaveTemplateInput,
  TemplateSummary,
} from '@/types';

import { call } from './api';

export const templateService = {
  /** 列出 `.knowledge/templates/*.md` */
  list: () => call<TemplateSummary[]>('templates_list'),

  /** 读取模板并替换占位符（{{title}} / {{date}} / {{time}} / {{datetime}} / {{weekday}}） */
  render: (input: RenderTemplateInput) => call<RenderedTemplate>('template_render', { input }),

  /** 把笔记正文另存为模板（写入 .knowledge/templates/<name>.md） */
  save: (input: SaveTemplateInput) => call<TemplateSummary>('template_save', { input }),
};

export type { RenderedTemplate, RenderTemplateInput, TemplateSummary };

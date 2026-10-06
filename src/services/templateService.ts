import type { RenderedTemplate, RenderTemplateInput, TemplateSummary } from '@/types';

import { call } from './api';

export const templateService = {
  /** 列出 `.knowledge/templates/*.md` */
  list: () => call<TemplateSummary[]>('templates_list'),

  /** 读取模板并替换占位符（{{title}} / {{date}} / {{time}} / {{datetime}} / {{weekday}}） */
  render: (input: RenderTemplateInput) => call<RenderedTemplate>('template_render', { input }),
};

export type { RenderedTemplate, RenderTemplateInput, TemplateSummary };

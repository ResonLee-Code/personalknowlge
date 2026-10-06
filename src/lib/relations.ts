/** 类型化关系的展示逻辑（N2）。 */

export const RELATION_TYPES = [
  'RELATED_TO',
  'DERIVED_FROM',
  'SUPERSEDES',
  'CONTRADICTS',
  'CONFIRMS',
  'DEPENDS_ON',
  'PART_OF',
  'EXAMPLE_OF',
  'REFERENCES',
] as const;

export type RelationType = (typeof RELATION_TYPES)[number];

export const RELATION_TYPE_LABELS: Record<RelationType, string> = {
  RELATED_TO: '相关',
  DERIVED_FROM: '派生自',
  SUPERSEDES: '取代',
  CONTRADICTS: '冲突',
  CONFIRMS: '印证',
  DEPENDS_ON: '依赖',
  PART_OF: '属于',
  EXAMPLE_OF: '示例',
  REFERENCES: '引用',
};

export type RelationTone = 'brand' | 'amber' | 'red' | 'slate';

/** 关系类型的展示文案与色调（SUPERSEDES/CONTRADICTS 需要更醒目）。 */
export function relationBadge(type: string): { label: string; tone: RelationTone } {
  const key = type.trim().toUpperCase().replace(/[\s-]+/g, '_') as RelationType;
  const label = RELATION_TYPE_LABELS[key] ?? type;
  const tone: RelationTone =
    key === 'SUPERSEDES' ? 'amber' : key === 'CONTRADICTS' ? 'red' : key === 'CONFIRMS' || key === 'DEPENDS_ON' ? 'brand' : 'slate';
  return { label, tone };
}

export const RELATION_TONE_CLASS: Record<RelationTone, string> = {
  brand: 'bg-[var(--brand-soft)] text-[var(--brand)]',
  amber: 'bg-amber-500/10 text-amber-600',
  red: 'bg-red-500/10 text-red-600',
  slate: 'bg-[var(--surface-muted)] text-[var(--ink-subtle)]',
};

export interface RelationLike {
  id: string;
  relationType: string;
  note?: string | null;
}

/** 元信息栏角标：被取代 / 取代他人 / 存在冲突。 */
export function relationHighlights(
  outgoing: RelationLike[],
  incoming: RelationLike[],
): {
  /** 谁取代了本篇（入边 SUPERSEDES） */
  supersededBy: RelationLike | null;
  /** 本篇取代了谁（出边 SUPERSEDES） */
  supersedes: RelationLike | null;
  conflictCount: number;
} {
  const byType = (list: RelationLike[], type: string) =>
    list.filter((item) => item.relationType.toUpperCase() === type);
  return {
    supersededBy: byType(incoming, 'SUPERSEDES')[0] ?? null,
    supersedes: byType(outgoing, 'SUPERSEDES')[0] ?? null,
    conflictCount:
      byType(outgoing, 'CONTRADICTS').length + byType(incoming, 'CONTRADICTS').length,
  };
}

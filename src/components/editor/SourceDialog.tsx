import { BookMarked } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input, Label } from '@/components/ui/input';
import { reportFailure } from '@/lib/actionFeedback';
import { SOURCE_TYPE_LABELS } from '@/lib/freshness';
import { errorMessage } from '@/lib/format';
import { noteService } from '@/services/noteService';
import type { Note } from '@/types';

/**
 * 来源与时效编辑（V1.1.2 N1）。
 *
 * 这些字段属于知识资产，会写回 Markdown frontmatter（source_type / source_ref /
 * source_reliability / valid_until），因此外部编辑器也能看到与修改。
 */
export function SourceDialog({
  note,
  onClose,
  onSaved,
}: {
  note: Note;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [sourceType, setSourceType] = useState(note.sourceType ?? '');
  const [sourceRef, setSourceRef] = useState(note.sourceRef ?? '');
  const [reliability, setReliability] = useState(note.sourceReliability ?? 0);
  const [validUntil, setValidUntil] = useState(note.validUntil?.slice(0, 10) ?? '');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      await noteService.setSource({
        id: note.id,
        sourceType: sourceType.trim() || null,
        sourceRef: sourceRef.trim() || null,
        sourceReliability: reliability > 0 ? reliability : null,
        validUntil: validUntil.trim() || null,
      });
      toast.success('已更新来源与时效');
      onSaved();
    } catch (error) {
      reportFailure('保存来源与时效失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent className="w-[min(520px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <BookMarked className="h-4 w-4 text-[var(--brand)]" />
            来源与时效
          </DialogTitle>
          <DialogDescription>
            记录这条知识从哪来、有多可信、什么时候需要复核。字段会写进笔记的 frontmatter。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="source-type">来源类型</Label>
            <select
              id="source-type"
              value={sourceType}
              onChange={(event) => setSourceType(event.target.value)}
              className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
            >
              <option value="">未标注</option>
              {Object.entries(SOURCE_TYPE_LABELS).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
              {/* 允许保留用户自定义值 */}
              {sourceType && !(sourceType in SOURCE_TYPE_LABELS) ? (
                <option value={sourceType}>{sourceType}</option>
              ) : null}
            </select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="source-ref">来源引用</Label>
            <Input
              id="source-ref"
              value={sourceRef}
              placeholder="https://… 或《书名》第 3 章"
              onChange={(event) => setSourceRef(event.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="source-reliability">可靠性（1–5）</Label>
            <select
              id="source-reliability"
              value={reliability}
              onChange={(event) => setReliability(Number(event.target.value))}
              className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
            >
              <option value={0}>未评估</option>
              <option value={1}>★☆☆☆☆ 个人推测</option>
              <option value={2}>★★☆☆☆ 参考价值有限</option>
              <option value={3}>★★★☆☆ 一般</option>
              <option value={4}>★★★★☆ 可信</option>
              <option value={5}>★★★★★ 官方/实测</option>
            </select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="valid-until">有效期至</Label>
            <Input
              id="valid-until"
              type="date"
              value={validUntil}
              onChange={(event) => setValidUntil(event.target.value)}
            />
            <p className="text-[11px] text-[var(--ink-subtle)]">
              留空表示长期有效；到期后会在这条笔记上提示「可能已过期」。
            </p>
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            保存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

import type {
  AskAnswer,
  AskNotePreview,
  AskQuestionInput,
  AskSaveInput,
  AskSaveResult,
} from '@/types';

import { call } from './api';

/**
 * Ask（V2.0 M1）：对当前笔记 / 分类 / 标签 / 全库提问。
 *
 * 隐私闸门、检索、模型降级和引用校验都在 Rust 服务层；前端只转发。
 */
export const askService = {
  ask: (input: AskQuestionInput) => call<AskAnswer>('ask_question', { input }),
  previewAnswerNote: (input: AskSaveInput) =>
    call<AskNotePreview>('ask_answer_note_preview', { input }),
  saveAnswer: (input: AskSaveInput) => call<AskSaveResult>('ask_answer_save', { input }),
};

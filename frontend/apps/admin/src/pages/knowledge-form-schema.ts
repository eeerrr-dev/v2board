import { z } from 'zod';

export const KNOWLEDGE_LOCALES = ['zh-CN', 'zh-TW', 'en-US', 'ja-JP', 'vi-VN', 'ko-KR'] as const;

export type KnowledgeLocale = (typeof KNOWLEDGE_LOCALES)[number];

const requiredText = (message: string) => z.string().trim().min(1, message);

export const knowledgeEditorSchema = z
  .object({
    id: z.number().int().positive().optional(),
    category: requiredText('分类不能为空'),
    language: z.enum(KNOWLEDGE_LOCALES, { error: '语言不能为空' }),
    title: requiredText('标题不能为空'),
    body: requiredText('内容不能为空'),
  })
  .strict();

export type KnowledgeEditorValues = z.input<typeof knowledgeEditorSchema>;
export type KnowledgeSavePayload = z.output<typeof knowledgeEditorSchema>;

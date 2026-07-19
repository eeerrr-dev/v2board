import { z } from 'zod';

export const KNOWLEDGE_LOCALES = ['zh-CN', 'zh-TW', 'en-US', 'ja-JP', 'vi-VN', 'ko-KR'] as const;

export type KnowledgeLocale = (typeof KNOWLEDGE_LOCALES)[number];

const requiredText = (message: string) => z.string().trim().min(1, message);

// Validation messages are i18n keys; FieldError resolves them through
// translateRuntimeMessage.
export const knowledgeEditorSchema = z
  .object({
    id: z.number().int().positive().optional(),
    category: requiredText('admin.knowledge.category_required'),
    language: z.enum(KNOWLEDGE_LOCALES, { error: 'admin.knowledge.language_required' }),
    title: requiredText('admin.knowledge.title_required'),
    body: requiredText('admin.knowledge.body_required'),
  })
  .strict();

export type KnowledgeEditorValues = z.input<typeof knowledgeEditorSchema>;
export type KnowledgeSavePayload = z.output<typeof knowledgeEditorSchema>;

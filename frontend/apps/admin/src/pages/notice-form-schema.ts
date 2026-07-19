import { z } from 'zod';
import { isHttpUrlInput } from '@/lib/form-input-validation';

const requiredText = (message: string) => z.string().trim().min(1, message);

// Validation messages are i18n keys; FieldError resolves them through
// translateRuntimeMessage.
export const noticeEditorSchema = z
  .object({
    id: z.number().int().positive().optional(),
    title: requiredText('admin.notices.title_required'),
    content: requiredText('admin.notices.content_required'),
    img_url: z
      .string()
      .trim()
      .refine((value) => value === '' || isHttpUrlInput(value), 'admin.notices.img_url_invalid'),
    tags: z.array(requiredText('admin.notices.tag_required')),
  })
  .strict()
  .transform((values) => ({
    ...(values.id === undefined ? {} : { id: values.id }),
    title: values.title,
    content: values.content,
    img_url: values.img_url || null,
    tags: values.tags.length > 0 ? [...new Set(values.tags)] : null,
  }));

export type NoticeEditorValues = z.input<typeof noticeEditorSchema>;
export type NoticeSavePayload = z.output<typeof noticeEditorSchema>;

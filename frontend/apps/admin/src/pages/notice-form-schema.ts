import { z } from 'zod';
import { isHttpUrlInput } from '@/lib/form-input-validation';

const requiredText = (message: string) => z.string().trim().min(1, message);

export const noticeEditorSchema = z
  .object({
    id: z.number().int().positive().optional(),
    title: requiredText('标题不能为空'),
    content: requiredText('内容不能为空'),
    img_url: z
      .string()
      .trim()
      .refine((value) => value === '' || isHttpUrlInput(value), '图片URL格式不正确'),
    tags: z.array(requiredText('标签不能为空')),
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

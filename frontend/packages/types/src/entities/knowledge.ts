export interface KnowledgeSummary {
  id: number;
  category: string;
  title: string;
  updated_at: number;
}

export interface Knowledge extends KnowledgeSummary {
  body: string;
  language: string;
  sort: number | null;
  show: 0 | 1;
  created_at: number;
}

export type KnowledgeCategory = Record<string, KnowledgeSummary[]>;

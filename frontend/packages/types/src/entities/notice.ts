export interface Notice {
  id: number;
  title: string;
  content: string;
  img_url: string | null;
  tags: string[] | null;
  show: 0 | 1;
  created_at: number;
  updated_at: number;
}

export interface NoticePage {
  data: Notice[];
  total: number;
}

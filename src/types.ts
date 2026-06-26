// TypeScript types matching Rust models

export interface Tool {
  id: number;
  name: string;
  global_path: string;
  project_rel_path: string;
  created_at: string;
  updated_at: string;
}

export interface Skill {
  id: number;
  name: string;
  description: string | null;
  source_path: string;
  content_hash: string;
  created_at: string;
  updated_at: string;
}

export interface ScanResult {
  skills_found: number;
  skills_new: number;
  skills_updated: number;
  errors: string[];
}

export interface Toast {
  type: "success" | "error";
  message: string;
  id: number;
}

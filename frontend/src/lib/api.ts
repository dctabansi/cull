export interface ColumnStats {
  name: string;
  inferred_type: string;
  total_count: number;
  null_count: number;
  unique_count: number;
  null_percentage: number;
  sample_values: string[];
}

export interface AuditReportRecord {
  job_id: string;
  filename: string;
  file_path: string;
  file_format: string;
  row_count: number;
  column_count: number;
  duplicate_row_count: number;
  columns: ColumnStats[];
  created_at: string;
}

export interface RowPreviewResponse {
  job_id: string;
  headers: string[];
  rows: string[][];
  offset: number;
  limit: number;
  total_rows: number;
}

export interface CleanJobRequest {
  drop_columns: string[];
  rename_map: Record<string, string>;
  deduplicate: boolean;
  drop_null_columns: string[];
}

export interface CleanJobResponse {
  job_id: string;
  output_filename: string;
  retained_row_count: number;
  removed_row_count: number;
}

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

export async function fetchAudits(): Promise<AuditReportRecord[]> {
  const res = await fetch(`${API_BASE}/api/v1/audits`, { cache: "no-store" });
  if (!res.ok) throw new Error(`Failed to fetch audits: ${res.statusText}`);
  return res.json();
}

export async function uploadFileForAudit(file: File): Promise<AuditReportRecord> {
  const formData = new FormData();
  formData.append("data", file);

  const res = await fetch(`${API_BASE}/api/v1/audits`, {
    method: "POST",
    body: formData,
  });
  if (!res.ok) {
    const errText = await res.text();
    throw new Error(`Upload failed: ${errText}`);
  }
  return res.json();
}

export async function fetchRowPreview(
  jobId: string,
  offset = 0,
  limit = 50
): Promise<RowPreviewResponse> {
  const res = await fetch(
    `${API_BASE}/api/v1/audits/${jobId}/preview?offset=${offset}&limit=${limit}`,
    { cache: "no-store" }
  );
  if (!res.ok) throw new Error(`Failed to load preview: ${res.statusText}`);
  return res.json();
}

export async function runCleanPipeline(
  jobId: string,
  payload: CleanJobRequest
): Promise<CleanJobResponse> {
  const res = await fetch(`${API_BASE}/api/v1/audits/${jobId}/clean`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) {
    const errText = await res.text();
    throw new Error(`Cleaning pipeline failed: ${errText}`);
  }
  return res.json();
}

export function getDownloadUrl(jobId: string): string {
  return `${API_BASE}/api/v1/audits/${jobId}/download`;
}

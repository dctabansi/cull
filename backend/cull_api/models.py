from datetime import datetime
from uuid import UUID

from pydantic import BaseModel, Field


class ColumnStatsSchema(BaseModel):
    name: str
    inferred_type: str
    total_count: int
    null_count: int
    unique_count: int
    null_percentage: float
    sample_values: list[str]


class AuditReportRecord(BaseModel):
    job_id: UUID
    filename: str
    file_path: str
    file_format: str
    row_count: int
    column_count: int
    duplicate_row_count: int
    columns: list[ColumnStatsSchema]
    created_at: datetime


class RowPreviewResponse(BaseModel):
    job_id: UUID
    headers: list[str]
    rows: list[list[str]]
    offset: int
    limit: int
    total_rows: int


class CleanJobRequest(BaseModel):
    drop_columns: list[str] = Field(default_factory=list)
    rename_map: dict[str, str] = Field(default_factory=dict)
    deduplicate: bool = False
    drop_null_columns: list[str] = Field(default_factory=list)


class CleanJobResponse(BaseModel):
    job_id: UUID
    output_filename: str
    retained_row_count: int
    removed_row_count: int

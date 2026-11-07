from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import Annotated
from uuid import UUID, uuid4

from cull_core import (
    audit_csv_file,
    audit_jsonl_file,
    clean_and_export_csv,
    preview_rows,
)
from litestar import Litestar, MediaType, Request, get, post
from litestar.config.cors import CORSConfig
from litestar.datastructures import UploadFile
from litestar.enums import RequestEncodingType
from litestar.exceptions import NotFoundException, ValidationException
from litestar.params import Body
from litestar.response import File as FileResponse
from litestar_granian import GranianPlugin

from backend.cull_api.config import settings
from backend.cull_api.models import (
    AuditReportRecord,
    CleanJobRequest,
    CleanJobResponse,
    ColumnStatsSchema,
    RowPreviewResponse,
)
from backend.cull_api.storage import WorkbenchStore

store = WorkbenchStore()


@asynccontextmanager
async def lifespan(app: Litestar) -> AsyncGenerator[None]:
    await store.connect()
    app.state.store = store
    try:
        yield
    finally:
        await store.close()


@get("/health")
async def health_check() -> dict[str, str]:
    return {
        "status": "ok",
        "postgres": "connected" if store.pg_pool is not None else "fallback",
        "valkey": "connected" if store.valkey is not None else "fallback",
    }


@post("/api/v1/audits")
async def upload_and_audit(
    data: Annotated[UploadFile, Body(media_type=RequestEncodingType.MULTI_PART)],
) -> AuditReportRecord:
    filename = data.filename or "uploaded.csv"
    suffix = Path(filename).suffix.lower()
    if suffix not in {".csv", ".jsonl", ".ndjson"}:
        raise ValidationException(
            detail=f"Unsupported file extension '{suffix}'. Expected .csv or .jsonl"
        )

    job_id = uuid4()
    settings.cull_upload_dir.mkdir(parents=True, exist_ok=True)
    saved_path = settings.cull_upload_dir / f"{job_id}{suffix}"

    content = await data.read()
    saved_path.write_bytes(content)

    raw_report = (
        audit_jsonl_file(str(saved_path))
        if suffix in {".jsonl", ".ndjson"}
        else audit_csv_file(str(saved_path))
    )

    record = AuditReportRecord(
        job_id=job_id,
        filename=filename,
        file_path=str(saved_path),
        file_format=raw_report.file_format,
        row_count=raw_report.row_count,
        column_count=raw_report.column_count,
        duplicate_row_count=raw_report.duplicate_row_count,
        columns=[
            ColumnStatsSchema(
                name=c.name,
                inferred_type=c.inferred_type,
                total_count=c.total_count,
                null_count=c.null_count,
                unique_count=c.unique_count,
                null_percentage=c.null_percentage,
                sample_values=c.sample_values,
            )
            for c in raw_report.columns
        ],
        created_at=datetime.now(UTC),
    )

    await store.save_audit(record)
    return record


@get("/api/v1/audits")
async def list_audits() -> list[AuditReportRecord]:
    return await store.list_audits()


@get("/api/v1/audits/{job_id:uuid}")
async def get_audit_detail(job_id: UUID) -> AuditReportRecord:
    record = await store.get_audit(job_id)
    if record is None:
        raise NotFoundException(detail=f"Audit job {job_id} not found")
    return record


@get("/api/v1/audits/{job_id:uuid}/preview")
async def get_audit_preview(
    job_id: UUID,
    offset: int = 0,
    limit: int = 100,
) -> RowPreviewResponse:
    record = await store.get_audit(job_id)
    if record is None:
        raise NotFoundException(detail=f"Audit job {job_id} not found")

    preview = preview_rows(record.file_path, offset=offset, limit=min(limit, 500))
    return RowPreviewResponse(
        job_id=job_id,
        headers=preview.headers,
        rows=preview.rows,
        offset=preview.offset,
        limit=preview.limit,
        total_rows=preview.total_rows,
    )


@post("/api/v1/audits/{job_id:uuid}/clean")
async def clean_audit_file(
    job_id: UUID,
    data: CleanJobRequest,
) -> CleanJobResponse:
    record = await store.get_audit(job_id)
    if record is None:
        raise NotFoundException(detail=f"Audit job {job_id} not found")
    if record.file_format != "csv":
        raise ValidationException(detail="Cleaning pipeline currently requires CSV format")

    output_filename = f"cleaned_{Path(record.filename).stem}.csv"
    output_path = settings.cull_upload_dir / f"{job_id}_cleaned.csv"

    retained = clean_and_export_csv(
        input_path=record.file_path,
        output_path=str(output_path),
        drop_columns=data.drop_columns,
        rename_map=data.rename_map,
        deduplicate=data.deduplicate,
        drop_null_columns=data.drop_null_columns,
    )

    return CleanJobResponse(
        job_id=job_id,
        output_filename=output_filename,
        retained_row_count=retained,
        removed_row_count=max(0, record.row_count - retained),
    )


@get("/api/v1/audits/{job_id:uuid}/download")
async def download_cleaned_file(job_id: UUID, request: Request) -> FileResponse:
    record = await store.get_audit(job_id)
    if record is None:
        raise NotFoundException(detail=f"Audit job {job_id} not found")

    cleaned_path = settings.cull_upload_dir / f"{job_id}_cleaned.csv"
    target_path = cleaned_path if cleaned_path.exists() else Path(record.file_path)

    return FileResponse(
        path=target_path,
        filename=f"cleaned_{record.filename}",
        media_type=MediaType.TEXT,
    )


cors_config = CORSConfig(
    allow_origins=["http://localhost:3000", "http://127.0.0.1:3000"],
    allow_methods=["GET", "POST", "DELETE", "OPTIONS"],
    allow_headers=["*"],
)

app = Litestar(
    route_handlers=[
        health_check,
        upload_and_audit,
        list_audits,
        get_audit_detail,
        get_audit_preview,
        clean_audit_file,
        download_cleaned_file,
    ],
    cors_config=cors_config,
    lifespan=[lifespan],
    plugins=[GranianPlugin()],
)

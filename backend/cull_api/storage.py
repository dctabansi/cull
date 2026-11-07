import json
from uuid import UUID

import asyncpg
from valkey.asyncio import Valkey

from backend.cull_api.config import settings
from backend.cull_api.models import AuditReportRecord

SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS audit_jobs (
    job_id UUID PRIMARY KEY,
    filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_format TEXT NOT NULL,
    row_count INTEGER NOT NULL,
    column_count INTEGER NOT NULL,
    duplicate_row_count INTEGER NOT NULL,
    report_payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
"""


class WorkbenchStore:
    """Manages PostgreSQL JSONB persistence and Valkey caching with in-memory fallback."""

    def __init__(self) -> None:
        self.pg_pool: asyncpg.Pool | None = None
        self.valkey: Valkey | None = None
        self._memory_fallback: dict[UUID, AuditReportRecord] = {}

    async def connect(self) -> None:
        settings.cull_upload_dir.mkdir(parents=True, exist_ok=True)
        try:
            self.pg_pool = await asyncpg.create_pool(
                dsn=settings.postgres_dsn,
                min_size=1,
                max_size=5,
                timeout=2.0,
            )
            async with self.pg_pool.acquire() as conn:
                await conn.execute(SCHEMA_SQL)
        except Exception:
            self.pg_pool = None

        try:
            client = Valkey.from_url(settings.valkey_url, decode_responses=True)
            await client.ping()
            self.valkey = client
        except Exception:
            self.valkey = None

    async def close(self) -> None:
        if self.pg_pool is not None:
            await self.pg_pool.close()
        if self.valkey is not None:
            await self.valkey.aclose()

    async def save_audit(self, record: AuditReportRecord) -> None:
        self._memory_fallback[record.job_id] = record
        payload_json = record.model_dump_json()

        if self.valkey is not None:
            await self.valkey.setex(
                f"cull:audit:{record.job_id}",
                settings.cache_ttl_seconds,
                payload_json,
            )

        if self.pg_pool is not None:
            async with self.pg_pool.acquire() as conn:
                await conn.execute(
                    """
                    INSERT INTO audit_jobs (
                        job_id, filename, file_path, file_format,
                        row_count, column_count, duplicate_row_count,
                        report_payload, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9)
                    """,
                    record.job_id,
                    record.filename,
                    record.file_path,
                    record.file_format,
                    record.row_count,
                    record.column_count,
                    record.duplicate_row_count,
                    payload_json,
                    record.created_at,
                )

    async def get_audit(self, job_id: UUID) -> AuditReportRecord | None:
        if self.valkey is not None:
            cached = await self.valkey.get(f"cull:audit:{job_id}")
            if cached:
                return AuditReportRecord.model_validate_json(cached)

        if self.pg_pool is not None:
            async with self.pg_pool.acquire() as conn:
                row = await conn.fetchrow(
                    "SELECT report_payload FROM audit_jobs WHERE job_id = $1",
                    job_id,
                )
                if row:
                    raw_payload = row["report_payload"]
                    data = json.loads(raw_payload) if isinstance(raw_payload, str) else raw_payload
                    return AuditReportRecord.model_validate(data)

        return self._memory_fallback.get(job_id)

    async def list_audits(self) -> list[AuditReportRecord]:
        if self.pg_pool is not None:
            async with self.pg_pool.acquire() as conn:
                rows = await conn.fetch(
                    "SELECT report_payload FROM audit_jobs ORDER BY created_at DESC LIMIT 50"
                )
                return [
                    AuditReportRecord.model_validate(
                        json.loads(r["report_payload"])
                        if isinstance(r["report_payload"], str)
                        else r["report_payload"]
                    )
                    for r in rows
                ]

        return sorted(
            self._memory_fallback.values(),
            key=lambda item: item.created_at,
            reverse=True,
        )

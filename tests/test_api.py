from pathlib import Path

import pytest
from litestar.testing import AsyncTestClient

from backend.cull_api.app import app
from backend.cull_api.config import settings


@pytest.mark.asyncio
async def test_upload_preview_and_clean_csv_workflow(tmp_path: Path) -> None:
    settings.cull_upload_dir = tmp_path / "uploads"

    sample_csv = (
        b"user_id,full_name,department,salary\n"
        b"101,Ada Lovelace,Engineering,145000\n"
        b"102,Grace Hopper,Engineering,152000\n"
        b"102,Grace Hopper,Engineering,152000\n"
        b"103,Alan Turing,,148000\n"
    )

    async with AsyncTestClient(app=app) as client:
        # 1. Upload and audit CSV
        resp = await client.post(
            "/api/v1/audits",
            files={"data": ("employees.csv", sample_csv, "text/csv")},
        )
        assert resp.status_code == 201
        audit = resp.json()
        job_id = audit["job_id"]
        assert audit["row_count"] == 4
        assert audit["duplicate_row_count"] == 1

        # 2. Fetch paginated preview
        preview_resp = await client.get(f"/api/v1/audits/{job_id}/preview?offset=0&limit=2")
        assert preview_resp.status_code == 200
        preview = preview_resp.json()
        assert preview["headers"] == ["user_id", "full_name", "department", "salary"]
        assert len(preview["rows"]) == 2
        assert preview["total_rows"] == 4

        # 3. Run cleaning pipeline (deduplicate + drop rows with null 'department' + rename column)
        clean_resp = await client.post(
            f"/api/v1/audits/{job_id}/clean",
            json={
                "drop_columns": ["salary"],
                "rename_map": {"full_name": "name"},
                "deduplicate": True,
                "drop_null_columns": ["department"],
            },
        )
        assert clean_resp.status_code == 201
        cleaned = clean_resp.json()
        assert cleaned["retained_row_count"] == 2
        assert cleaned["removed_row_count"] == 2

        # 4. Download cleaned CSV and verify contents
        dl_resp = await client.get(f"/api/v1/audits/{job_id}/download")
        assert dl_resp.status_code == 200
        lines = dl_resp.text.strip().splitlines()
        assert lines[0] == "user_id,name,department"
        assert len(lines) == 3

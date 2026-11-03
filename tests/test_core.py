from pathlib import Path

from cull_core import audit_csv_file, audit_jsonl_file


def test_audit_csv_schema_and_duplicates(tmp_path: Path) -> None:
    csv_file = tmp_path / "customers.csv"
    csv_file.write_text(
        "id,email,balance,active\n"
        "1,alice@example.com,120.50,true\n"
        "2,bob@example.com,,false\n"
        "2,bob@example.com,,false\n"
        "3,charlie@example.com,89.99,true\n",
        encoding="utf-8",
    )

    report = audit_csv_file(str(csv_file))

    assert report.file_format == "csv"
    assert report.row_count == 4
    assert report.column_count == 4
    assert report.duplicate_row_count == 1

    cols = {c.name: c for c in report.columns}
    assert cols["id"].inferred_type == "Integer"
    assert cols["email"].inferred_type == "Text"
    assert cols["balance"].inferred_type == "Float"
    assert cols["balance"].null_count == 2
    assert cols["balance"].null_percentage == 50.0
    assert cols["active"].inferred_type == "Boolean"


def test_audit_jsonl_schema_and_missing_keys(tmp_path: Path) -> None:
    jsonl_file = tmp_path / "events.jsonl"
    jsonl_file.write_text(
        '{"event_id": 101, "service": "auth", "latency_ms": 14.2}\n'
        '{"event_id": 102, "service": "billing", "latency_ms": null}\n'
        '{"event_id": 103, "service": "auth"}\n',
        encoding="utf-8",
    )

    report = audit_jsonl_file(str(jsonl_file))

    assert report.file_format == "jsonl"
    assert report.row_count == 3
    assert report.column_count == 3
    assert report.duplicate_row_count == 0

    cols = {c.name: c for c in report.columns}
    assert cols["event_id"].inferred_type == "Integer"
    assert cols["latency_ms"].inferred_type == "Float"
    assert cols["latency_ms"].null_count == 2

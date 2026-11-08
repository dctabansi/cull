"use client";

import React, { useEffect, useState } from "react";
import {
  AuditReportRecord,
  CleanJobResponse,
  RowPreviewResponse,
  fetchAudits,
  fetchRowPreview,
  getDownloadUrl,
  runCleanPipeline,
  uploadFileForAudit,
} from "@/lib/api";

export default function CullWorkbenchPage() {
  const [audits, setAudits] = useState<AuditReportRecord[]>([]);
  const [activeAudit, setActiveAudit] = useState<AuditReportRecord | null>(null);
  const [preview, setPreview] = useState<RowPreviewResponse | null>(null);
  const [pageOffset, setPageOffset] = useState<number>(0);
  const pageLimit = 25;

  // Cleaning pipeline state
  const [dropColumns, setDropColumns] = useState<Record<string, boolean>>({});
  const [dropNullColumns, setDropNullColumns] = useState<Record<string, boolean>>({});
  const [renameMap, setRenameMap] = useState<Record<string, string>>({});
  const [deduplicate, setDeduplicate] = useState<boolean>(true);
  const [cleanResult, setCleanResult] = useState<CleanJobResponse | null>(null);

  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    fetchAudits()
      .then((list) => {
        setAudits(list);
        if (list.length > 0) {
          selectAudit(list[0], 0);
        }
      })
      .catch(() => {
        // API may not have jobs yet
      });
  }, []);

  async function selectAudit(record: AuditReportRecord, offset = 0) {
    setActiveAudit(record);
    setPageOffset(offset);
    setCleanResult(null);
    setDropColumns({});
    setDropNullColumns({});
    setRenameMap({});
    setErrorMsg(null);

    try {
      const prev = await fetchRowPreview(record.job_id, offset, pageLimit);
      setPreview(prev);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : "Failed to load row preview");
    }
  }

  async function handleFileUpload(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;

    setIsLoading(true);
    setErrorMsg(null);
    try {
      const created = await uploadFileForAudit(file);
      setAudits((prev) => [created, ...prev]);
      await selectAudit(created, 0);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setIsLoading(false);
    }
  }

  async function handlePageChange(newOffset: number) {
    if (!activeAudit) return;
    setPageOffset(newOffset);
    try {
      const prev = await fetchRowPreview(activeAudit.job_id, newOffset, pageLimit);
      setPreview(prev);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : "Failed to paginate preview");
    }
  }

  async function handleExecuteClean() {
    if (!activeAudit) return;
    setIsLoading(true);
    setErrorMsg(null);

    const dropped = Object.entries(dropColumns)
      .filter(([, checked]) => checked)
      .map(([col]) => col);

    const nonNulls = Object.entries(dropNullColumns)
      .filter(([, checked]) => checked)
      .map(([col]) => col);

    const activeRenames: Record<string, string> = {};
    for (const [orig, target] of Object.entries(renameMap)) {
      if (target && target.trim() !== "" && target.trim() !== orig) {
        activeRenames[orig] = target.trim();
      }
    }

    try {
      const result = await runCleanPipeline(activeAudit.job_id, {
        drop_columns: dropped,
        rename_map: activeRenames,
        deduplicate,
        drop_null_columns: nonNulls,
      });
      setCleanResult(result);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : "Cleaning pipeline error");
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <div style={{ display: "grid", gridTemplateColumns: "280px 1fr", minHeight: "100vh" }}>
      {/* Left Sidebar: File Upload & Job History */}
      <aside
        style={{
          background: "var(--panel)",
          borderRight: "1px solid var(--border)",
          padding: "1.25rem",
          display: "flex",
          flexDirection: "column",
          gap: "1.25rem",
        }}
      >
        <div>
          <h1 style={{ fontSize: "1.35rem", fontWeight: 700, letterSpacing: "-0.02em" }}>
            cull <span style={{ color: "var(--accent)", fontSize: "0.85rem" }}>v0.1.0</span>
          </h1>
          <p style={{ color: "var(--muted)", fontSize: "0.8rem" }}>
            Clean, Unify, Lint, Load Workbench
          </p>
        </div>

        <label
          style={{
            display: "block",
            border: "1px dashed var(--accent)",
            borderRadius: "6px",
            padding: "1rem",
            textAlign: "center",
            cursor: "pointer",
            background: "rgba(56, 189, 248, 0.06)",
            fontSize: "0.85rem",
          }}
        >
          <strong>{isLoading ? "Processing..." : "+ Upload CSV / JSONL"}</strong>
          <input
            type="file"
            accept=".csv,.jsonl,.ndjson"
            onChange={handleFileUpload}
            style={{ display: "none" }}
          />
        </label>

        <div style={{ flex: 1, overflowY: "auto" }}>
          <h2
            style={{
              fontSize: "0.75rem",
              textTransform: "uppercase",
              color: "var(--muted)",
              marginBottom: "0.5rem",
            }}
          >
            Recent Audit Jobs ({audits.length})
          </h2>
          <div style={{ display: "flex", flexDirection: "column", gap: "0.4rem" }}>
            {audits.map((job) => {
              const isSelected = activeAudit?.job_id === job.job_id;
              return (
                <button
                  key={job.job_id}
                  onClick={() => selectAudit(job, 0)}
                  style={{
                    textAlign: "left",
                    padding: "0.6rem 0.75rem",
                    borderRadius: "6px",
                    border: "1px solid",
                    borderColor: isSelected ? "var(--accent)" : "var(--border)",
                    background: isSelected ? "var(--panel-alt)" : "transparent",
                    color: "var(--text)",
                  }}
                >
                  <div style={{ fontWeight: 600, fontSize: "0.85rem" }}>{job.filename}</div>
                  <div style={{ fontSize: "0.75rem", color: "var(--muted)" }}>
                    {job.row_count.toLocaleString()} rows · {job.column_count} cols (
                    {job.file_format.toUpperCase()})
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      </aside>

      {/* Main Workbench Area */}
      <main style={{ padding: "1.5rem 2rem", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
        {errorMsg && (
          <div
            style={{
              padding: "0.75rem 1rem",
              background: "rgba(248, 113, 113, 0.12)",
              border: "1px solid var(--danger)",
              borderRadius: "6px",
              color: "var(--danger)",
              fontSize: "0.875rem",
            }}
          >
            {errorMsg}
          </div>
        )}

        {!activeAudit ? (
          <div
            style={{
              border: "1px solid var(--border)",
              borderRadius: "8px",
              padding: "3rem",
              textAlign: "center",
              color: "var(--muted)",
            }}
          >
            Upload a CSV or JSONL file from the sidebar to run native Rust schema inference, null
            anomaly detection, and interactive row cleaning.
          </div>
        ) : (
          <>
            {/* Top Summary Banner */}
            <section
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(4, 1fr)",
                gap: "1rem",
              }}
            >
              <div style={{ background: "var(--panel)", padding: "1rem", borderRadius: "6px", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: "0.75rem", color: "var(--muted)" }}>Source File</div>
                <div style={{ fontSize: "1.1rem", fontWeight: 700 }}>{activeAudit.filename}</div>
              </div>
              <div style={{ background: "var(--panel)", padding: "1rem", borderRadius: "6px", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: "0.75rem", color: "var(--muted)" }}>Total Rows</div>
                <div style={{ fontSize: "1.1rem", fontWeight: 700 }}>
                  {activeAudit.row_count.toLocaleString()}
                </div>
              </div>
              <div style={{ background: "var(--panel)", padding: "1rem", borderRadius: "6px", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: "0.75rem", color: "var(--muted)" }}>Columns Audited</div>
                <div style={{ fontSize: "1.1rem", fontWeight: 700 }}>{activeAudit.column_count}</div>
              </div>
              <div style={{ background: "var(--panel)", padding: "1rem", borderRadius: "6px", border: "1px solid var(--border)" }}>
                <div style={{ fontSize: "0.75rem", color: "var(--muted)" }}>Exact Duplicate Rows</div>
                <div
                  style={{
                    fontSize: "1.1rem",
                    fontWeight: 700,
                    color: activeAudit.duplicate_row_count > 0 ? "var(--danger)" : "var(--success)",
                  }}
                >
                  {activeAudit.duplicate_row_count.toLocaleString()}
                </div>
              </div>
            </section>

            {/* Schema Profiler & Column Transformation Rules */}
            <section
              style={{
                background: "var(--panel)",
                border: "1px solid var(--border)",
                borderRadius: "8px",
                padding: "1.25rem",
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: "1rem",
                }}
              >
                <h2 style={{ fontSize: "1rem", fontWeight: 600 }}>
                  Column Schema Audit & Cleaning Rules
                </h2>
                {activeAudit.file_format === "csv" && (
                  <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
                    <label style={{ fontSize: "0.85rem", display: "flex", alignItems: "center", gap: "0.4rem" }}>
                      <input
                        type="checkbox"
                        checked={deduplicate}
                        onChange={(e) => setDeduplicate(e.target.checked)}
                      />
                      Deduplicate exact rows
                    </label>
                    <button
                      onClick={handleExecuteClean}
                      disabled={isLoading}
                      style={{
                        background: "var(--accent)",
                        color: "#000",
                        fontWeight: 600,
                        border: "none",
                        padding: "0.45rem 0.9rem",
                        borderRadius: "5px",
                        fontSize: "0.85rem",
                      }}
                    >
                      Run Rust Clean Pipeline
                    </button>
                  </div>
                )}
              </div>

              {cleanResult && (
                <div
                  style={{
                    marginBottom: "1rem",
                    padding: "0.75rem 1rem",
                    background: "rgba(52, 211, 153, 0.1)",
                    border: "1px solid var(--success)",
                    borderRadius: "6px",
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    fontSize: "0.85rem",
                  }}
                >
                  <span>
                    Cleaned file ready: <strong>{cleanResult.retained_row_count}</strong> rows
                    retained ({cleanResult.removed_row_count} anomalous/duplicate rows removed).
                  </span>
                  <a
                    href={getDownloadUrl(activeAudit.job_id)}
                    style={{
                      color: "var(--accent)",
                      fontWeight: 600,
                      textDecoration: "underline",
                    }}
                  >
                    Download {cleanResult.output_filename}
                  </a>
                </div>
              )}

              <div style={{ overflowX: "auto" }}>
                <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.85rem" }}>
                  <thead>
                    <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left", color: "var(--muted)" }}>
                      <th style={{ padding: "0.5rem" }}>Column</th>
                      <th style={{ padding: "0.5rem" }}>Inferred Type</th>
                      <th style={{ padding: "0.5rem" }}>Unique</th>
                      <th style={{ padding: "0.5rem" }}>Nulls (%)</th>
                      <th style={{ padding: "0.5rem" }}>Samples</th>
                      <th style={{ padding: "0.5rem" }}>Rename To</th>
                      <th style={{ padding: "0.5rem" }}>Drop Null Rows</th>
                      <th style={{ padding: "0.5rem" }}>Drop Column</th>
                    </tr>
                  </thead>
                  <tbody>
                    {activeAudit.columns.map((col) => (
                      <tr key={col.name} style={{ borderBottom: "1px solid var(--border)" }}>
                        <td style={{ padding: "0.55rem 0.5rem", fontWeight: 600 }}>{col.name}</td>
                        <td style={{ padding: "0.55rem 0.5rem", color: "var(--accent)" }}>
                          {col.inferred_type}
                        </td>
                        <td style={{ padding: "0.55rem 0.5rem" }}>{col.unique_count}</td>
                        <td
                          style={{
                            padding: "0.55rem 0.5rem",
                            color: col.null_count > 0 ? "var(--danger)" : "var(--muted)",
                          }}
                        >
                          {col.null_count} ({col.null_percentage}%)
                        </td>
                        <td style={{ padding: "0.55rem 0.5rem", color: "var(--muted)" }}>
                          {col.sample_values.slice(0, 3).join(", ") || "—"}
                        </td>
                        <td style={{ padding: "0.55rem 0.5rem" }}>
                          <input
                            type="text"
                            placeholder={col.name}
                            value={renameMap[col.name] || ""}
                            onChange={(e) =>
                              setRenameMap((prev) => ({ ...prev, [col.name]: e.target.value }))
                            }
                          />
                        </td>
                        <td style={{ padding: "0.55rem 0.5rem" }}>
                          <input
                            type="checkbox"
                            checked={!!dropNullColumns[col.name]}
                            onChange={(e) =>
                              setDropNullColumns((prev) => ({
                                ...prev,
                                [col.name]: e.target.checked,
                              }))
                            }
                          />
                        </td>
                        <td style={{ padding: "0.55rem 0.5rem" }}>
                          <input
                            type="checkbox"
                            checked={!!dropColumns[col.name]}
                            onChange={(e) =>
                              setDropColumns((prev) => ({
                                ...prev,
                                [col.name]: e.target.checked,
                              }))
                            }
                          />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>

            {/* Paginated Data Grid Preview */}
            {preview && (
              <section
                style={{
                  background: "var(--panel)",
                  border: "1px solid var(--border)",
                  borderRadius: "8px",
                  padding: "1.25rem",
                }}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginBottom: "0.85rem",
                  }}
                >
                  <h2 style={{ fontSize: "1rem", fontWeight: 600 }}>
                    Row Data Inspector ({pageOffset + 1}–
                    {Math.min(pageOffset + preview.rows.length, preview.total_rows)} of{" "}
                    {preview.total_rows.toLocaleString()})
                  </h2>
                  <div style={{ display: "flex", gap: "0.5rem" }}>
                    <button
                      disabled={pageOffset === 0}
                      onClick={() => handlePageChange(Math.max(0, pageOffset - pageLimit))}
                      style={{
                        padding: "0.3rem 0.7rem",
                        background: "var(--panel-alt)",
                        border: "1px solid var(--border)",
                        color: "var(--text)",
                        borderRadius: "4px",
                        fontSize: "0.8rem",
                      }}
                    >
                      Previous
                    </button>
                    <button
                      disabled={pageOffset + pageLimit >= preview.total_rows}
                      onClick={() => handlePageChange(pageOffset + pageLimit)}
                      style={{
                        padding: "0.3rem 0.7rem",
                        background: "var(--panel-alt)",
                        border: "1px solid var(--border)",
                        color: "var(--text)",
                        borderRadius: "4px",
                        fontSize: "0.8rem",
                      }}
                    >
                      Next
                    </button>
                  </div>
                </div>

                <div style={{ overflowX: "auto" }}>
                  <table style={{ width: "100%", borderCollapse: "collapse", fontSize: "0.8rem" }}>
                    <thead>
                      <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left" }}>
                        <th style={{ padding: "0.45rem", color: "var(--muted)" }}>#</th>
                        {preview.headers.map((h) => (
                          <th key={h} style={{ padding: "0.45rem" }}>
                            {h}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {preview.rows.map((row, idx) => (
                        <tr key={idx} style={{ borderBottom: "1px solid var(--border)" }}>
                          <td style={{ padding: "0.45rem", color: "var(--muted)" }}>
                            {pageOffset + idx + 1}
                          </td>
                          {row.map((cell, cIdx) => (
                            <td
                              key={cIdx}
                              style={{
                                padding: "0.45rem",
                                color: cell === "" ? "var(--danger)" : "var(--text)",
                                fontStyle: cell === "" ? "italic" : "normal",
                              }}
                            >
                              {cell === "" ? "NULL" : cell}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>
            )}
          </>
        )}
      </main>
    </div>
  );
}

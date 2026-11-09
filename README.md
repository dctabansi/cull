# cull — Clean, Unify, Lint, Load

**cull** is a self-hosted, full-stack tabular and document ETL auditing and schema migration workbench. It pairs a native Rust parsing and transformation engine (`cull-core` via PyO3/Maturin) with an asynchronous Python API (`Litestar` + `Granian`) and a `Next.js` web interface to inspect, profile, deduplicate, and clean large CSV and JSONL files before loading them into production PostgreSQL databases.

## Who It's For

Backend engineers, database administrators, and data operations teams who frequently receive messy CSV or JSONL exports from third-party vendors, legacy systems, or operational dumps and need fast, deterministic schema inference, null/anomaly detection, and repeatable cleaning pipelines without spinning up heavy distributed clusters.

## Architecture & Tech Stack

- **Native Compute Core (`crates/cull-core`):** Written in Rust (2024 edition) and bridged to Python via `PyO3` and `Maturin`. Performs single-pass schema inference (`Integer`, `Float`, `Boolean`, `Text`, `Empty`), null token detection, exact row deduplication, paginated row streaming, and projected CSV transformations.
- **Backend API (`backend/cull_api`):** Built on Python 3.15 with `Litestar` and served via `Granian` (`litestar-granian`). Persists file audit reports as `JSONB` documents in **PostgreSQL 17** (`asyncpg`) and caches active job profiles in **Valkey 8** (`valkey[libvalkey]`).
- **Web Workbench (`frontend`):** Built with **Next.js** and **TypeScript**. Provides drag-and-drop file ingestion, column-level anomaly and cardinality cards, a paginated row inspector highlighting null values, and interactive column pruning, renaming, and deduplication controls.
- **Task Runner & Tooling:** Managed with `uv`, `cargo`, `just`, `Ruff`, and `Clippy`.

## Getting Started

### Prerequisites

- **Rust** (stable toolchain via `rustup`)
- **Python 3.15+** and **uv**
- **Node.js 22+** and `npm`
- **Docker** & **Docker Compose** (optional for local PostgreSQL 17 and Valkey 8; the API includes an automatic in-memory store fallback for local testing)
- **just** command runner

### Installation & Local Development

1. **Clone the repository and configure environment variables:**
   ```bash
   git clone [https://github.com/dctabansi/cull.git](https://github.com/dctabansi/cull.git)
   cd cull
   cp .env.example .env

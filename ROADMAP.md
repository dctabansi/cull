# Cull Project Roadmap

This roadmap tracks upcoming engineering work for `cull` across `v0.2.0` and `v1.0.0`, focusing on expanding format support in the Rust `cull-core` engine, expressive schema validation rules, direct database loading, and saved transformation recipes.

---

## Milestone 1: `v0.2.0` — Format Expansion & Validation Engine (~16 Hours)

- [ ] **Apache Parquet & Multi-Sheet Excel (`.xlsx`) Ingestion & Export (`cull-core`)** *(~4 hrs)*
  - Integrate `parquet` / `arrow` and `calamine` crates into `cull-core` so users can audit, preview, and convert Parquet and Excel workbooks alongside CSV and JSONL.
- [ ] **Declarative Column Lint Rules & Constraint Engine (`cull-core` + `Litestar` + UI)** *(~5 hrs)*
  - Allow users to define column-level validation rules in the UI (custom regex patterns, numeric min/max bounds, allowed enum sets, and ISO-8601 date parsing) and generate downloadable quarantine reports of non-conforming rows.
- [ ] **High-Speed Fuzzy Deduplication via String Similarity (`cull-core`)** *(~4 hrs)*
  - Implement Jaro-Winkler and normalized Levenshtein distance clustering in Rust with `rayon` parallelism to identify near-duplicate records across configurable key columns.
- [ ] **JSONL & Nested Document Flattening Pipeline** *(~3 hrs)*
  - Extend `clean_and_export_csv` to support JSONL inputs, automatically flattening nested JSON objects via dot-notation keys into normalized tabular columns.

---

## Milestone 2: `v1.0.0` — Database Loading, Saved Recipes & Production Hardening (~14 Hours)

- [ ] **Reusable Transformation Recipes (`JSONB` Persistence & Replay UI)** *(~4 hrs)*
  - Save sequences of column renames, drops, deduplication toggles, and lint rules as named PostgreSQL `JSONB` recipes so recurring weekly file drops can be cleaned in one click.
- [ ] **Direct PostgreSQL Table Loader (`COPY` & Upsert Target)** *(~4 hrs)*
  - Build a connection manager and schema mapper allowing users to load cleaned datasets directly into a target PostgreSQL table using high-throughput `COPY` or `ON CONFLICT DO UPDATE`.
- [ ] **Virtualized Data Grid & Column Distribution Histograms (`frontend`)** *(~3 hrs)*
  - Upgrade the row inspector to a virtualized grid capable of smooth scrolling over 100k+ rows and render inline SVG value distribution histograms inside each column header.
- [ ] **Production Docker Multi-Stage Build & End-to-End Playwright Suite** *(~3 hrs)*
  - Add a multi-stage `Dockerfile` compiling `cull-core`, `Litestar/Granian`, and `Next.js` for Cloud Run deployment, paired with automated Playwright UI tests and GitHub Actions CI.

# Typst Service

Rust service that renders Markdown or raw Typst projects into PDF through a REST API.

## Features

- `POST /v1/render/pdf` for synchronous PDF generation
- `POST /v1/jobs` plus polling endpoints for asynchronous jobs
- `POST /v1/templates` and `GET /v1/templates` for template management
- API key authentication with `x-api-key`
- Local artifact/template storage and isolated per-job work directories
- Native Typst rendering through Rust libraries instead of the external CLI
- Local Typst package loading from `APP_PACKAGES_DIR`

## Structure

- `src/handlers`: HTTP routes and middleware
- `src/services`: application services and job orchestration
- `src/models`: request, template, and job models
- `src/repositories`: filesystem-backed artifact, workspace, and template repositories
- `src/infra`: Typst renderer and app state wiring
- `src/config`: application configuration
- `src/utils`: shared error, markdown, and telemetry helpers

## Run

```bash
cargo run
```

Use `APP_API_KEYS`, `APP_STORAGE_ROOT`, `APP_FONTS_DIR`, and `APP_PACKAGES_DIR` to configure the service.

## Notes

- Runtime PDF compilation now uses the Typst Rust libraries directly.
- The repo ships with a default `report` template under `assets/templates/report`.
- Local Typst packages are expected under a Typst-style directory layout such as `assets/packages/preview/my-package/0.1.0`.

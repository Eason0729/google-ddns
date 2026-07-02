# Build commands

- Build: `cargo build` / `cargo build --release`
- Lint/format: `cargo fmt --check` (fix with `cargo fmt`)
- No test framework configured yet.

# Security

- The service-account key JSON (`*.json` except `config.example.json`) and
  `config.json` are gitignored. Never commit credentials. The container image
  must not bake in any key; mount them at `/config/service-account.json` at
  runtime.
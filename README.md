# google-ddns

A minimal, low-resource dynamic DNS updater for Google Cloud DNS.

A single self-contained binary keeps your A/AAAA records (including wildcards)
in sync with the host's public IPv4/IPv6 on a configurable interval. No async
runtime, no SDK — just raw HTTPS and a hand-signed JWT.

## Features

- **IPv4 and IPv6** — per-record `ip_source` (`v4` / `v6`).
- **Wildcard records** — GCP supports them natively; just use `*.example.com.`.
- **User-specified TTL and interval**.
- **Low resource** — blocking I/O, ~7 MB stripped binary, ~9 MB rootless image.
- **Multi-arch** — `linux/amd64` and `linux/arm64` from one build.
- **File-based config** — a single JSON file.

## Quick start

```bash
docker run -d --name ddns \
  -v /path/to/config:/config:ro \
  ghcr.io/eason0729/google-ddns:latest
```

Mount two files at `/config`:

1. `service-account.json` — a GCP service-account key with the
   **DNS Administrator** (or `roles/dns.admin`) role on your managed zone.
2. `config.json` — see [`config.example.json`](./config.example.json).

```json
{
  "credentials_file": "/config/service-account.json",
  "managed_zone": "my-zone",
  "interval_secs": 300,
  "records": [
    { "name": "ddns.example.com.", "ttl": 300, "ip_source": "v4" },
    { "name": "*.example.com.",    "ttl": 300, "ip_source": "v6" }
  ]
}
```

The path is set via `CONFIG_FILE` (default `/config/config.json`) or the first
CLI argument. The GCP project ID is read from the service-account key, so it
does not appear in the config. `ip_source` (`v4` or `v6`) implies the record
type (`A` or `AAAA`); only the IP version actually used by some record is
fetched.

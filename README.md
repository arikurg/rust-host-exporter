# host-exporter

A small Prometheus exporter for host CPU and memory, written in Rust with a
hand-rolled HTTP server and the standard-library thread pool. No async runtime,
no web framework: the standard library plus `sysinfo` for readings and `clap`
for the CLI.

Built as an exercise in Rust ownership, borrowing, and thread-safe shared state.

## What it exposes

Scraping `/metrics` returns the standard Prometheus text format:

```
# HELP exporter_up Whether the exporter is running.
# TYPE exporter_up gauge
exporter_up 1
# HELP host_cpu_usage_percent Overall CPU usage across all cores.
# TYPE host_cpu_usage_percent gauge
host_cpu_usage_percent 18.36
# HELP host_memory_total_bytes Total physical memory.
# TYPE host_memory_total_bytes gauge
host_memory_total_bytes 8589934592
# HELP host_memory_used_bytes Physical memory in use.
# TYPE host_memory_used_bytes gauge
host_memory_used_bytes 6777520128
# HELP host_memory_available_bytes Physical memory available to processes.
# TYPE host_memory_available_bytes gauge
host_memory_available_bytes 0
```

CPU usage is measured as the delta between consecutive scrapes, matching how
`node_exporter` behaves. On macOS the `available` figure reads `0` because the
OS has no direct equivalent of Linux's `MemAvailable`; derive free memory as
`total - used` if you need it there.

## Build and run

```sh
cargo run                      # listens on 127.0.0.1:9100
cargo run -- --port 9200       # custom port
cargo run -- --host 0.0.0.0    # accept scrapes from other machines
cargo build --release          # optimized, stripped binary in target/release/
```

### Flags

| Flag     | Default     | Purpose                                   |
| -------- | ----------- | ----------------------------------------- |
| `--host` | `127.0.0.1` | Bind address; `0.0.0.0` for remote scrapes |
| `--port` | `9100`      | TCP port to listen on                     |

## Endpoints

| Path       | Response                                    |
| ---------- | ------------------------------------------- |
| `/metrics` | Prometheus text-format metrics              |
| `/healthz` | `ok` for liveness checks                    |
| any other  | `404 Not Found`                             |

## Wiring it to Prometheus

Prometheus pulls: it scrapes this exporter on its own schedule. Point it at the
exporter with a scrape job (see `prometheus.yml` in this repo):

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: "host-exporter"
    static_configs:
      - targets: ["localhost:9100"]
```

Run Prometheus against that config, then open its UI at `http://localhost:9090`
and query the metrics:

```
host_cpu_usage_percent
host_memory_used_bytes / host_memory_total_bytes * 100
```

## Notes

The server spawns one thread per connection and guards the shared `sysinfo`
handle behind an `Arc<Mutex<System>>`, so concurrent scrapes are serialized on
the reads and safe by construction.
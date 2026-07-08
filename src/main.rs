// Prometheus exporter. Milestone 5: configurable bind address via a clap
// CLI, and real request routing. The request line we used to throw away
// now decides whether a client gets /metrics, /healthz, or a 404.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use clap::Parser;
use sysinfo::System;

/// A tiny Prometheus host-metrics exporter.
#[derive(Parser)]
#[command(name = "exporter", version, about)]
struct Args {
    /// Address to bind to (use 0.0.0.0 to accept remote scrapes).
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// TCP port to listen on.
    #[arg(long, default_value_t = 9100)]
    port: u16,
}

struct Sample {
    cpu_percent: f32,
    mem_total_bytes: u64,
    mem_used_bytes: u64,
    mem_available_bytes: u64,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);

    let listener = TcpListener::bind(&addr)?;
    println!("exporter listening on http://{addr}/metrics");

    let mut initial = System::new();
    initial.refresh_cpu_usage();
    let sys = Arc::new(Mutex::new(initial));

    for stream in listener.incoming() {
        let stream = stream?;
        let sys = Arc::clone(&sys);
        thread::spawn(move || {
            if let Err(e) = handle_connection(stream, sys) {
                eprintln!("connection error: {e}");
            }
        });
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, sys: Arc<Mutex<System>>) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf)?;

    // Turn the raw bytes into text and read only the first line, e.g.
    // "GET /metrics HTTP/1.1". Lossy conversion so garbage can't panic us.
    let request = String::from_utf8_lossy(&buf[..n]);
    let request_line = request.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    // One match decides status, content type, and body for every route.
    let (status, content_type, body) = match (method, path) {
        ("GET", "/metrics") => {
            let sample = collect(&sys);
            (
                "200 OK",
                "text/plain; version=0.0.4",
                render_metrics(&sample),
            )
        }
        ("GET", "/healthz") => ("200 OK", "text/plain", "ok\n".to_string()),
        _ => ("404 Not Found", "text/plain", "not found\n".to_string()),
    };

    let content_length = body.len();
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {content_length}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}"
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn collect(sys: &Mutex<System>) -> Sample {
    let mut sys = sys.lock().expect("system mutex was poisoned");

    sys.refresh_cpu_usage();
    sys.refresh_memory();

    Sample {
        cpu_percent: sys.global_cpu_usage(),
        mem_total_bytes: sys.total_memory(),
        mem_used_bytes: sys.used_memory(),
        mem_available_bytes: sys.available_memory(),
    }
}

fn render_metrics(s: &Sample) -> String {
    let mut out = String::new();

    out.push_str("# HELP exporter_up Whether the exporter is running.\n");
    out.push_str("# TYPE exporter_up gauge\n");
    out.push_str("exporter_up 1\n");

    out.push_str("# HELP host_cpu_usage_percent Overall CPU usage across all cores.\n");
    out.push_str("# TYPE host_cpu_usage_percent gauge\n");
    out.push_str(&format!("host_cpu_usage_percent {:.2}\n", s.cpu_percent));

    out.push_str("# HELP host_memory_total_bytes Total physical memory.\n");
    out.push_str("# TYPE host_memory_total_bytes gauge\n");
    out.push_str(&format!("host_memory_total_bytes {}\n", s.mem_total_bytes));

    out.push_str("# HELP host_memory_used_bytes Physical memory in use.\n");
    out.push_str("# TYPE host_memory_used_bytes gauge\n");
    out.push_str(&format!("host_memory_used_bytes {}\n", s.mem_used_bytes));

    out.push_str("# HELP host_memory_available_bytes Physical memory available to processes.\n");
    out.push_str("# TYPE host_memory_available_bytes gauge\n");
    out.push_str(&format!(
        "host_memory_available_bytes {}\n",
        s.mem_available_bytes
    ));

    out
}
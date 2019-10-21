#![warn(rust_2018_idioms)]
use failure::{bail, format_err, Error};
use fastcgi_client::{empty, Client, Params};
use log::{error, trace};
use opcache::Opcache;
use prometheus_exporter_base::{render_prometheus, MetricType, PrometheusMetric};
use std::net::SocketAddr;
use tokio;
use tokio::net::TcpStream;

mod opcache;

#[derive(Debug, Clone, Default)]
struct MyOptions {}

enum MetricValue {
    Bool(bool),
    Int(isize),
    Float(f64),
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let addr = ([0, 0, 0, 0], 32221).into();
    println!("starting exporter on {}", addr);

    render_prometheus(addr, MyOptions::default(), |request, options| {
        async move {
            trace!(
                "Incoming Request: \n(request == {:?}, options == {:?})",
                request,
                options
            );

            let op = match fetch_opcache_stats().await {
                Ok(stats) => stats,
                Err(e) => {
                    error!("Error: {}", e.as_fail());
                    error!("Caused by: {}", e.backtrace());
                    unimplemented!();
                }
            };

            let pc: Vec<(PrometheusMetric<'_>, MetricValue)> = vec![
                (
                    PrometheusMetric::new("opcache_enabled", MetricType::Gauge, "Opcache Enabled"),
                    MetricValue::Bool(op.opcache_enabled),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_cache_full",
                        MetricType::Gauge,
                        "Opcache Cache Full",
                    ),
                    MetricValue::Bool(op.cache_full),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_restart_pending",
                        MetricType::Gauge,
                        "Opcache Restart Pending",
                    ),
                    MetricValue::Bool(op.restart_pending),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_restart_in_progress",
                        MetricType::Gauge,
                        "Opcache Restart In Progress",
                    ),
                    MetricValue::Bool(op.restart_in_progress),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_memory_usage_used_memory",
                        MetricType::Gauge,
                        "Opcache Used Memory",
                    ),
                    MetricValue::Int(op.memory_usage.used_memory),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_memory_usage_free_memory",
                        MetricType::Gauge,
                        "Opcache Free Memory",
                    ),
                    MetricValue::Int(op.memory_usage.free_memory),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_memory_usage_wasted_memory",
                        MetricType::Gauge,
                        "Opcache Wasted Memory",
                    ),
                    MetricValue::Int(op.memory_usage.wasted_memory),
                ),
                (
                    PrometheusMetric::new(
                        "opcache_memory_usage_current_wasted_percentage",
                        MetricType::Gauge,
                        "Opcache Wasted Memory Percentage",
                    ),
                    MetricValue::Float(op.memory_usage.current_wasted_percentage),
                ),
            ];

            let giant_string = pc
                .iter()
                .map(|(metric, value)| {
                    let mut s = metric.render_header();
                    match value {
                        MetricValue::Bool(v) => s.push_str(&metric.render_sample(None, *v as i32)),
                        MetricValue::Int(v) => s.push_str(&metric.render_sample(None, *v as i32)),
                        MetricValue::Float(v) => s.push_str(&metric.render_sample(None, *v)),
                    };
                    s
                })
                .collect::<Vec<String>>()
                .join("\n");
            // let mut s = pc.render_header();

            // let mut attributes = Vec::new();
            // attributes.push(("folder", "/var/log/"));
            // s.push_str(&pc.render_sample(Some(&attributes), 2));

            Ok(giant_string)
        }
    })
    .await
    .unwrap();
}

async fn fetch_opcache_stats() -> Result<Opcache, Error> {
    let addr: SocketAddr = "127.0.0.1:9000".parse()?;
    let stream = TcpStream::connect(&addr).await?;

    let mut client = Client::new(stream, false);

    let params = Params::with_predefine()
        .set_request_method("POST")
        .set_document_root("/var/www")
        .set_script_name("index.php")
        .set_script_filename("/var/www/index.php")
        .set_request_uri("/opcache")
        .set_document_uri("/opcache")
        .set_remote_addr("127.0.0.1")
        .set_remote_port("12345")
        .set_server_addr("127.0.0.1")
        .set_server_port("80")
        .set_server_name("opcache-exporter/0.1")
        .set_content_type("")
        .set_content_length("0");
    let output = client
        .do_request(&params, &mut empty())
        .await
        .map_err(|_| format_err!("Bad"))?
        .get_stdout()
        .unwrap();

    let response_string = String::from_utf8(output)?;
    let response_vec = &response_string.splitn(2, "\r\n\r\n").collect::<Vec<&str>>();
    match &response_vec.as_slice() {
        [_headers, body] => {
            let opcache: Opcache = serde_json::from_str(body)?;
            Ok(opcache)
        }
        response => bail!("Error parsing fastcgi response: {:#?}", response),
    }
}

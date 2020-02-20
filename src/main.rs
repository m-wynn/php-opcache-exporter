#![warn(rust_2018_idioms)]
use failure::{bail, format_err, Error};
use fastcgi_client::{Client, Params};
use log::{error, trace};
use opcache::Opcache;
use prometheus_exporter_base::{render_prometheus, MetricType, PrometheusMetric};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use tokio;
use structopt::StructOpt;

mod opcache;

#[derive(StructOpt, Debug, Clone)]
struct ScraperOptions {
  #[structopt(short, long, env = "LISTEN_ADDR", default_value="127.0.0.1:8080")]
  listen_addr: SocketAddr,
  #[structopt(flatten)]
  fastcgi_options: FastcgiOptions
}

#[derive(StructOpt, Debug, Clone)]
struct FastcgiOptions {
  #[structopt(short, long, env = "FASTCGI_ADDR", default_value="127.0.0.1:9000")]
  fastcgi_addr: SocketAddr,
  #[structopt(long, env = "FASTCGI_REQUEST_METHOD", default_value="GET", possible_values=&["GET", "POST"])]
  request_method: String,
  #[structopt(long, env = "FASTCGI_SCRIPT_FILENAME", default_value="/var/www/index.php")]
  script_filename: PathBuf,
  #[structopt(long, env = "FASTCGI_REQUEST_URI", default_value="/opcache")]
  request_uri: String,
  #[structopt(long, env = "FASTCGI_DOCUMENT_URI")]
  document_uri: Option<String>,
  #[structopt(long, env = "FASTCGI_REMOTE_ADDR", default_value="127.0.0.1:12345")]
  remote_addr: SocketAddr,
  #[structopt(long, env = "FASTCGI_SERVER_ADDR", default_value="127.0.0.1:80")]
  server_addr: SocketAddr,
  #[structopt(long, env = "FASTCGI_SERVER_NAME", default_value="example.com")]
  server_name: String,
}

enum MetricValue {
    Bool(bool),
    Int(isize),
    Float(f64),
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let opt = ScraperOptions::from_args();

    println!("starting exporter on {}, scraping fastcgi on {}", &opt.listen_addr, &opt.fastcgi_options.fastcgi_addr);


    render_prometheus(
        opt.listen_addr,
        opt,
        move |request, options| async move {
            trace!(
                "Incoming Request: \n(request == {:?}, options == {:?})",
                request,
                options
            );

            match fetch_opcache_stats(&options.fastcgi_options).await {
                Ok(stats) => render_opcache_stats(stats),
                Err(e) => {
                    error!("Error: {}", e.as_fail());
                    let metric = PrometheusMetric::new(
                            "opcache_up",
                            MetricType::Gauge,
                            "Opcache Scrape Successful");
                    let s = format!("{}{}",metric.render_header(), metric.render_sample(None, false as i32));
                    Ok(s)
                }
            }
        }
    )
    .await;
}

async fn fetch_opcache_stats(options: &FastcgiOptions) -> Result<Opcache, Error> {
    let stream = TcpStream::connect(&options.fastcgi_addr)?;

    let mut client = Client::new(stream, false);
    let document_root = options.script_filename.parent().and_then(|path| path.to_str()).unwrap_or("/");
    let script_name = options.script_filename.file_name().and_then(|name| name.to_str()).unwrap_or("index.php");
    let script_filename = options.script_filename.to_str().unwrap_or("index.php");
    let document_uri = options.document_uri.as_ref().unwrap_or(&options.request_uri);

    let remote_addr = options.remote_addr.ip().to_string();
    let remote_port = options.remote_addr.port().to_string();

    let server_addr = options.server_addr.ip().to_string();
    let server_port = options.server_addr.port().to_string();

    let mut params = Params::with_predefine()
        .set_request_method(&options.request_method)
        .set_document_root(document_root)
        .set_script_name(script_name)
        .set_script_filename(&script_filename)
        .set_request_uri(&options.request_uri)
        .set_document_uri(&document_uri)
        .set_remote_addr(&remote_addr)
        .set_remote_port(&remote_port)
        .set_server_addr(&server_addr)
        .set_server_port(&server_port)
        .set_server_name(&options.server_name)
        .set_content_type("")
        .set_content_length("0");

    params.insert("HTTP_HOST", &options.server_name);

    let output = client
        .do_request(&params, &mut std::io::empty())
        .map_err(|e| format_err!("Could not complete fastcgi request: {}", e.description()))?
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

fn render_opcache_stats(op: Opcache) -> Result<String, Error> {
    let pc: Vec<(PrometheusMetric<'_>, MetricValue)> = vec![
        (
            PrometheusMetric::new(
                "opcache_up",
                MetricType::Gauge,
                "Opcache Scrape Successful",
            ),
            MetricValue::Bool(true),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_enabled",
                MetricType::Gauge,
                "Opcache Enabled",
            ),
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
        (
            PrometheusMetric::new(
                "opcache_interned_strings_usage_buffer_size",
                MetricType::Gauge,
                "Opcache Interned Strings Buffer Size",
            ),
            MetricValue::Int(op.interned_strings_usage.buffer_size),
        ),
        (
            PrometheusMetric::new(
                "opcache_interned_strings_usage_used_memory",
                MetricType::Gauge,
                "Opcache Interned Strings Used Memory",
            ),
            MetricValue::Int(op.interned_strings_usage.used_memory),
        ),
        (
            PrometheusMetric::new(
                "opcache_interned_strings_usage_free_memory",
                MetricType::Gauge,
                "Opcache Interned Strings Free Memory",
            ),
            MetricValue::Int(op.interned_strings_usage.free_memory),
        ),
        (
            PrometheusMetric::new(
                "opcache_interned_strings_usage_number_of_strings",
                MetricType::Gauge,
                "Opcache Interned Strings Number of Strings",
            ),
            MetricValue::Int(op.interned_strings_usage.number_of_strings),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_num_cached_scripts",
                MetricType::Gauge,
                "Opcache Cached Scripts",
            ),
            MetricValue::Int(op.opcache_statistics.num_cached_scripts),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_num_cached_keys",
                MetricType::Gauge,
                "Opcache Cached Keys",
            ),
            MetricValue::Int(op.opcache_statistics.num_cached_keys),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_max_cached_keys",
                MetricType::Gauge,
                "Opcache Max Cached Keys",
            ),
            MetricValue::Int(op.opcache_statistics.max_cached_keys),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_hits",
                MetricType::Counter,
                "Opcache Hits",
            ),
            MetricValue::Int(op.opcache_statistics.max_cached_keys),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_start_time",
                MetricType::Gauge,
                "Opcache Start Time",
            ),
            MetricValue::Int(op.opcache_statistics.start_time),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_last_restart_time",
                MetricType::Gauge,
                "Opcache Last Restart Time",
            ),
            MetricValue::Int(op.opcache_statistics.last_restart_time),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_oom_restarts",
                MetricType::Counter,
                "Opcache OOM Restarts",
            ),
            MetricValue::Int(op.opcache_statistics.oom_restarts),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_hash_restarts",
                MetricType::Counter,
                "Opcache Hash Restarts",
            ),
            MetricValue::Int(op.opcache_statistics.hash_restarts),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_manual_restarts",
                MetricType::Counter,
                "Opcache Manual Restarts",
            ),
            MetricValue::Int(op.opcache_statistics.manual_restarts),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_misses",
                MetricType::Counter,
                "Opcache Misses",
            ),
            MetricValue::Int(op.opcache_statistics.misses),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_blacklist_misses",
                MetricType::Counter,
                "Opcache Blacklist Misses",
            ),
            MetricValue::Int(op.opcache_statistics.blacklist_misses),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_blacklist_miss_ratio",
                MetricType::Gauge,
                "Opcache Blacklist Miss Ratio",
            ),
            MetricValue::Float(op.opcache_statistics.blacklist_miss_ratio),
        ),
        (
            PrometheusMetric::new(
                "opcache_opcache_statistics_opcache_hit_rate",
                MetricType::Gauge,
                "Opcache Hit Rate",
            ),
            MetricValue::Float(op.opcache_statistics.opcache_hit_rate),
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

    Ok(giant_string)
}

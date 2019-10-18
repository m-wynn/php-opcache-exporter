use prometheus_exporter_base::{render_prometheus, MetricType, PrometheusMetric};
use tokio::net::TcpStream;
use fastcgi_client::{Client, Params};
use std::env::current_dir;
use std::net::SocketAddr;
use std::path::PathBuf;
use failure::{Error, format_err};

#[derive(Debug, Clone, Default)]
struct MyOptions {}

fn main() {
    let addr = ([0, 0, 0, 0], 32221).into();
    println!("starting exporter on {}", addr);

    render_prometheus(addr, MyOptions::default(), |request, options| {
        async move {
            println!(
                "in our render_prometheus(request == {:?}, options == {:?})",
                request, options
            );

            let opcache_stats = fetch_opcache_stats().await?;

            let pc =
                PrometheusMetric::new("folder_size", MetricType::Counter, "Size of the folder");
            let mut s = pc.render_header();

            let mut attributes = Vec::new();
            attributes.push(("folder", "/var/log/"));
            s.push_str(&pc.render_sample(Some(&attributes), 2));

            Ok(s)
        }
    });
}

async fn fetch_opcache_stats() -> Result<(), Error> {
    let addr: SocketAddr = "127.0.0.1:9000".parse()?;
    let stream = TcpStream::connect(&addr).await?;

    let mut client = Client::new(stream, false);

    // let document_root = "/var/www/";
    // let script_name = "/var/www/post.php";

    // let document_root = current_dir().unwrap().join("tests").join("php");
    // let document_root = document_root.to_str().unwrap();
    // let script_name = current_dir().unwrap().join("tests").join("php").join("post.php");
    // let script_name = script_name.to_str().unwrap();

    let body = b"p1=3&p2=4";
    let len = format!("{}", body.len());

    let params = Params::with_predefine()
        .set_request_method("POST")
        // .set_document_root(document_root)
        .set_script_name("/opcache.php")
        // .set_script_filename(script_name)
        .set_request_uri("/opcache.php")
        .set_document_uri("/opcache.php")
        .set_remote_addr("127.0.0.1")
        .set_remote_port("12345")
        .set_server_addr("127.0.0.1")
        .set_server_port("80")
        .set_server_name("opcache-exporter")
        .set_content_type("")
        .set_content_length("0");
    let output = client.do_request(&params, &mut &body[..]).await.map_err(|_| format_err!("Bad"))?;

    let stdout = dbg!(String::from_utf8(output.get_stdout().unwrap_or(Default::default()))?);
    Ok(())
}

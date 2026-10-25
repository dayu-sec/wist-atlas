use atlas_app::{MonolithRunResult, TopologyAppBuilder, parse_monolith_input};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, input) = match parse_monolith_input(&args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    let builder = TopologyAppBuilder::new().with_mode(mode);
    let app = match mode {
        atlas_app::MonolithMode::PostgresLive => match builder.build_async().await {
            Ok(app) => app,
            Err(err) => {
                eprintln!("wist-atlas monolith failed to initialize: {err}");
                std::process::exit(1);
            }
        },
        _ => match builder.build() {
            Ok(app) => app,
            Err(err) => {
                eprintln!("wist-atlas monolith failed to initialize: {err}");
                std::process::exit(1);
            }
        },
    };

    let result = app.run_async(input).await;

    match result {
        Ok(MonolithRunResult::Single(summary)) => {
            println!("wist-atlas monolith started");
            println!("ingest_id={}", summary.ingest_id);
            println!("host={}", summary.host_name);
            if let Some(network_name) = summary.network_name.as_deref() {
                println!("network={network_name}");
            }
            if let Some(assoc_ip) = summary.assoc_ip.as_deref() {
                println!("ip={assoc_ip}");
            }
            if !summary.responsibilities.is_empty() {
                println!("responsibilities={}", summary.responsibilities.join(","));
            }
        }
        Ok(MonolithRunResult::Replay(summary)) => {
            println!("wist-atlas replay finished");
            println!("lines_total={}", summary.total_lines);
            println!("lines_ok={}", summary.success_lines);
            println!("lines_failed={}", summary.failed_lines);
            println!("hosts={}", summary.host_count);
            println!("networks={}", summary.network_count);
            println!("processes={}", summary.process_count);
            println!("processes_enriched={}", summary.enriched_process_count);
            println!("host_runtimes={}", summary.host_runtime_count);
            if let Some(ingest_id) = summary.last_ingest_id.as_deref() {
                println!("last_ingest_id={ingest_id}");
            }
            for failure in summary.failures.iter().take(5) {
                println!("failure={failure}");
            }
        }
        Ok(MonolithRunResult::Reset(message)) => {
            println!("wist-atlas reset finished");
            println!("status={message}");
        }
        Ok(MonolithRunResult::ExportVisualization(summary)) => {
            println!("wist-atlas visualization export finished");
            println!("output={}", summary.output_path.display());
            println!("hosts={}", summary.host_count);
            println!("processes={}", summary.process_count);
        }
        Ok(MonolithRunResult::PrintJson(body)) => {
            println!("{body}");
        }
        Ok(MonolithRunResult::Serve { listen }) => {
            println!("wist-atlas http server stopped");
            println!("listen={listen}");
        }
        Err(err) => {
            eprintln!("wist-atlas monolith failed: {err}");
            std::process::exit(1);
        }
    }
}

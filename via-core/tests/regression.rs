use reqwest::Client;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const SERVER_BIN: &str = "target/release/gatekeeper.exe"; // Windows extension
const PORT: u16 = 3000;
const URL: &str = "http://127.0.0.1:3000/ingest";

#[tokio::test]
async fn test_ingestion_throughput_invariant() {
    // 1. Build Server
    println!("Building gatekeeper...");
    let status = Command::new("cargo")
        .args(&["build", "--release", "--bin", "gatekeeper"])
        .status()
        .expect("Failed to build gatekeeper");
    assert!(status.success());

    // 2. Start Server
    println!("Starting gatekeeper...");
    let mut server = Command::new(SERVER_BIN)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start gatekeeper");

    // Wait for startup
    thread::sleep(Duration::from_secs(2));

    // 3. Run Load Test (Small burst)
    let client = Client::new();
    let count = 5000;
    
    println!("Sending {} requests...", count);
    
    let start = std::time::Instant::now();
    let mut tasks = Vec::new();

    for i in 0..count {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::json!({
                "u": format!("user_{}", i),
                "v": 100.0,
                "t": 1234567890
            });
            client.post(URL).json(&body).send().await
        }));
    }

    let mut success = 0;
    for task in tasks {
        if let Ok(Ok(res)) = task.await {
            if res.status().is_success() {
                success += 1;
            }
        }
    }

    let duration = start.elapsed();
    let eps = count as f64 / duration.as_secs_f64();
    
    println!("Completed {}/{} requests in {:.2?} ({:.0} EPS)", success, count, duration, eps);

    // 4. Assertions
    assert_eq!(success, count, "Dropped events under low load!");
    assert!(eps > 1000.0, "Throughput too low for invariant test!");

    // 5. Cleanup
    server.kill().expect("Failed to kill server");
}

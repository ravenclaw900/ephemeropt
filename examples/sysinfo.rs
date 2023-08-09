use ephemeropt::EphemeralOption;
use std::time::Duration;
use sysinfo::{CpuExt, System, SystemExt};
use tokio::sync::{mpsc, oneshot};

enum SystemRequest {
    Cpu(oneshot::Sender<f32>),
    // Other possible system queries here
}

fn system_task() -> mpsc::Sender<SystemRequest> {
    let mut sys = System::new();
    let mut cpu_opt = EphemeralOption::new_empty(Duration::from_secs(1));
    let (tx, mut rx) = mpsc::channel(10);
    tokio::task::spawn(async move {
        while let Some(request) = rx.recv().await {
            match request {
                SystemRequest::Cpu(channel) => {
                    if let Some(&cpu) = cpu_opt.get() {
                        eprintln!("using cached CPU data");
                        channel.send(cpu).unwrap();
                    } else {
                        sys.refresh_cpu();
                        let cpu = sys.global_cpu_info().cpu_usage();
                        channel.send(cpu).unwrap();
                        cpu_opt.insert(cpu);
                    }
                }
            };
        }
    });
    tx
}

#[tokio::main]
async fn main() {
    let sys_tx = system_task();

    for _ in 0..3 {
        let sys_tx = sys_tx.clone();
        tokio::spawn(async move {
            loop {
                let (resp_tx, resp_rx) = oneshot::channel();
                sys_tx.send(SystemRequest::Cpu(resp_tx)).await.unwrap();
                println!("CPU usage: {}", resp_rx.await.unwrap());
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

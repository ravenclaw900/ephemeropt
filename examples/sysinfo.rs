use ephemeropt::EphemeralOption;
use std::time::Duration;
use sysinfo::{CpuExt, System, SystemExt};
use tokio::sync::{mpsc, oneshot};

fn cpu_task() -> mpsc::Sender<oneshot::Sender<f32>> {
    let mut sys = System::new();
    let mut cpu_opt = EphemeralOption::new_empty(Duration::from_secs(1));
    let (tx, mut rx) = mpsc::channel::<oneshot::Sender<f32>>(10);
    tokio::task::spawn(async move {
        while let Some(channel) = rx.recv().await {
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
    });
    tx
}

#[tokio::main]
async fn main() {
    let sys_tx = cpu_task();

    for _ in 0..3 {
        let sys_tx = sys_tx.clone();
        tokio::spawn(async move {
            loop {
                let (resp_tx, resp_rx) = oneshot::channel();
                sys_tx.send(resp_tx).await.unwrap();
                println!("CPU usage: {}", resp_rx.await.unwrap());
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

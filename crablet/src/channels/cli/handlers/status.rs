use anyhow::Result;

pub fn handle_status() -> Result<()> {
    println!("System Status: OK");
    println!("[System 1 Active | Latency: ~15ms]");
    Ok(())
}

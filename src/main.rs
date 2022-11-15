mod sync_device_code_workflow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    sync_device_code_workflow::example_sync_devicecode_workflow()?;
    Ok(())
}

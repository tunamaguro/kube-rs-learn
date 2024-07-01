use controller::controller;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let controller = controller::run();

    controller.await;

    Ok(())
}

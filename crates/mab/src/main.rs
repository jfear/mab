use anyhow::Result;

fn main() {
    tracing_subscriber::fmt::init();

    if let Err(err) = run() {
        tracing::error!(error = %err, "application failed");
        std::process::exit(1);
    }
}

// `run` will gain fallible operations as the application grows. The return
// type is kept now so error handling patterns are already in place.
#[expect(clippy::unnecessary_wraps)]
fn run() -> Result<()> {
    let version = mab_core::VERSION;
    println!("Mab v{version} - My Alignment Browser");

    Ok(())
}

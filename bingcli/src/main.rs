use anyhow::Result;
use bingcli::BingCliApp;

fn main() -> Result<()> {
    // Initialize app
    let mut app = BingCliApp::new()?;
    app.initialize()?;
    
    println!("BingTray started successfully!");
    
    // Run the CLI menu
    app.run()?;
    
    Ok(())
}

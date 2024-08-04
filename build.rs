use std::{ env, io };
use winresource::WindowsResource;


fn main() -> io::Result<()> {
    if cfg!(debug_assertions) {
        return Ok(());
    }

    // https://stackoverflow.com/questions/30291757/attaching-an-icon-resource-to-a-rust-application
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut wr = WindowsResource::new();
        println!("Setting icon...");
        wr.set_icon("build_assets/icon.ico");
        println!("Compiling WindowsResource...");
        wr.compile()?;
    }


    Ok(())
}


#[path = "rail_viewer/mod.rs"]
mod rail_viewer;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().unwrap_or_else(|| "rail-viewer.html".into());

    if args.next().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: cargo run --example rail_viewer -- [output.html]",
        ));
    }

    let (world, trace) = rail_viewer::scenario();

    std::fs::write(&path, rail_viewer::render(&world, &trace))?;

    println!(
        "Open {} in a browser.",
        std::path::Path::new(&path).display()
    );
    Ok(())
}

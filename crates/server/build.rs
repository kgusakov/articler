use std::{env, process::Command};

use static_files::resource_dir;

const TAILWIND_REBUILD_VAR: &str = "TAILWIND_REBUILD";
const TAILWIND_BIN_VAR: &str = "TAILWIND_BIN";

fn main() -> std::io::Result<()> {
    // TODO Rebuild only when *.hbs or input.css changes
    // Tailwind css rebuild
    if let Ok(flag) = env::var(TAILWIND_REBUILD_VAR)
        && flag == "1"
    {
        let tailwind_bin = env::var(TAILWIND_BIN_VAR).unwrap_or_else(|_| {
            panic!(
                "With enabled {TAILWIND_REBUILD_VAR} you must set {TAILWIND_BIN_VAR} env variable"
            )
        });

        let output = Command::new(tailwind_bin)
            .args(["-i", "tailwind/input.css", "-o", "static/css/main.css"])
            .output()?;

        if output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
        } else {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));

            return Err(std::io::Error::other("Tailwind processing failed"));
        }
    }

    // Static resources
    let mut res = resource_dir("./static");
    res.with_generated_fn("static_resources");
    res.build()?;

    Ok(())
}

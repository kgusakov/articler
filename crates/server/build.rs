use static_files::resource_dir;

fn main() -> std::io::Result<()> {
    let mut res = resource_dir("./static");
    res.with_generated_fn("static_resources");
    res.build()
}

mod args;
mod gpu;
mod render;

fn main() {
    let list: Vec<String> = std::env::args().skip(1).collect();
    let cmd = list.first().cloned().unwrap_or_default();
    let args = args::Args::new(list);
    let r = match cmd.as_str() {
        "render" => render::render(&args),
        _ => Err("usage: claude-vivarium-rs render ...".to_string()),
    };
    if let Err(e) = r {
        eprintln!("claude-vivarium: {e}");
        std::process::exit(1);
    }
}

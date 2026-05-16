mod tui;

use clap::Parser;

#[derive(Parser)]
#[command(name = "litm-app", about = "Lost in the Mess — mesh operator TUI")]
struct Args {
    #[arg(long)]
    id: u32,
    #[arg(long, short, default_value = "litm")]
    password: String,
    #[arg(long, short, default_value = "wlan0")]
    iface: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tui::run(args.id, &args.password, &args.iface).await
}

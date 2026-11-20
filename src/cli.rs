use clap::Parser;

use crate::theme::ThemeName;

/// Pulse: a terminal system monitor.
#[derive(Parser, Debug)]
#[command(name = "pulse", version, about)]
pub struct Cli {
    /// Refresh interval in milliseconds (200-10000).
    #[arg(
        short,
        long,
        default_value_t = 1000,
        value_parser = clap::value_parser!(u64).range(200..=10_000)
    )]
    pub refresh: u64,

    /// Colour theme.
    #[arg(short, long, value_enum, default_value_t = ThemeName::Dark)]
    pub theme: ThemeName,

    /// Skip the start-up animation.
    #[arg(long)]
    pub no_splash: bool,
}

#![allow(dead_code)]

mod agent;
mod cli;
mod commands;
mod freeze;
mod parser;
mod validation;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    cli::run()
}

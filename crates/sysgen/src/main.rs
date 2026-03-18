#![allow(dead_code)]

mod agent;
mod cli;
mod commands;
mod freeze;
mod parser;
mod traceability;
mod validation;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    cli::run()
}

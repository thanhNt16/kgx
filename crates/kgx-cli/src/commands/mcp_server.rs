pub fn run(_json: bool, transport: &str) -> anyhow::Result<()> {
    if transport != "stdio" {
        anyhow::bail!("only --transport stdio supported");
    }
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(kgx_mcp::server::serve_stdio(std::env::current_dir()?))?;
    Ok(())
}

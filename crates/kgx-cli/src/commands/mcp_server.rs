pub fn run(_json: bool, transport: &str) -> anyhow::Result<()> {
    if transport != "stdio" {
        anyhow::bail!("only --transport stdio supported");
    }
    let rt = tokio::runtime::Runtime::new()?;
    // Pass the cwd (not an eagerly-resolved vault root): serve_stdio resolves
    // .brain/ lazily per message so the server starts from any directory and
    // initialize/tools-list always succeed.
    rt.block_on(kgx_mcp::server::serve_stdio(std::env::current_dir()?))?;
    Ok(())
}

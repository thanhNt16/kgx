pub fn run(transport: &str, port: u16) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    match transport {
        "http" => {
            rt.block_on(kgx_mcp::http::serve(port))?;
        }
        "stdio" => {
            rt.block_on(kgx_mcp::server::serve_stdio(std::env::current_dir()?))?;
        }
        other => {
            anyhow::bail!("unsupported transport: {other}. Use 'stdio' or 'http'.")
        }
    }
    Ok(())
}

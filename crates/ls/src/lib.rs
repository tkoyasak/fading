use tower_lsp_server::{LspService, Server};

mod backend;
mod code_actions;
mod diagnostics;
mod document;

pub fn run() -> std::io::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(serve());
    Ok(())
}

async fn serve() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(backend::Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}

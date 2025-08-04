use tower_lsp_server::{LspService, Server};

mod lsp;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(lsp::Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}

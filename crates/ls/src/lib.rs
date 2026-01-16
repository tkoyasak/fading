use tower_lsp_server::{LspService, Server};

mod backend;
mod document;
mod metadata;

pub async fn start() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(backend::Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}

use lapce_plugin::{
    psp_types::{
        lsp_types::{
            request::{Formatting, Initialize},
            DocumentFormattingParams, InitializeParams, InitializeResult, OneOf,
            ServerCapabilities,
        },
        ExecuteProcessResult, Request,
    },
    register_plugin, LapcePlugin, PLUGIN_RPC,
};
use serde_json::Value;

#[derive(Default, Debug)]
struct State {
    prettier_path: Option<String>,
}

register_plugin!(State);

impl State {
    fn handle_init(&mut self, params: &InitializeParams) -> Result<Value, String> {
        if let Some(opts) = params.initialization_options.as_ref() {
            if let Some(server_path) = opts.get("prettierPath") {
                if let Some(server_path) = server_path.as_str() {
                    if !server_path.is_empty() {
                        self.prettier_path = Some(server_path.to_string())
                    }
                }
            }
        }

        let server_params = InitializeResult {
            capabilities: ServerCapabilities {
                document_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        };

        serde_json::to_value(server_params).map_err(|s| s.to_string())
    }

    fn handle_formatting(&self, params: &DocumentFormattingParams) -> Result<Value, String> {
        let file_path = params
            .text_document
            .uri
            .to_file_path()
            .map_err(|_| format!("'{}' is not a valid file path", params.text_document.uri))?
            .to_str()
            .ok_or(format!(
                "'{}' is not a valid unicode string",
                params.text_document.uri
            ))?
            .to_string();

        let prettier_path = self.prettier_path.clone().unwrap_or("prettier".to_string());

        // Hack because we can't send the document in the result
        std::thread::sleep(std::time::Duration::from_secs(2));
        PLUGIN_RPC
            .execute_process(
                prettier_path,
                vec![
                    "--write".to_string(), // Remove this once `execute_process` supports stdout/stderr
                    file_path,
                ],
            )
            .map(|ExecuteProcessResult { success }| Value::Bool(success))
            .map_err(|e| e.to_string())
    }
}

impl LapcePlugin for State {
    fn handle_request(&mut self, id: u64, method: String, client_params: Value) {
        #[allow(clippy::single_match)]
        match method.as_str() {
            Initialize::METHOD => {
                let params: InitializeParams = serde_json::from_value(client_params).unwrap();
                match self.handle_init(&params) {
                    Ok(ok) => PLUGIN_RPC.host_success(id, ok).unwrap(),
                    Err(err) => PLUGIN_RPC.host_error(id, err).unwrap(),
                };
            }
            Formatting::METHOD => {
                let params =
                    serde_json::from_value::<DocumentFormattingParams>(client_params).unwrap();
                match self.handle_formatting(&params) {
                    Ok(ok) => PLUGIN_RPC.host_success(id, ok).unwrap(),
                    Err(err) => PLUGIN_RPC.host_error(id, err).unwrap(),
                }
            }
            _ => PLUGIN_RPC
                .host_error(
                    id,
                    format!("Prettier plugin does not support method '{method}'"),
                )
                .unwrap(),
        }
    }
}

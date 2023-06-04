// Deny usage of print and eprint as it won't have same result
// use the functions provided by PLUGIN_RPC
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]

use anyhow::Result;
use lapce_plugin::{
    psp_types::{
        lsp_types::{
            request::Initialize, DocumentFilter, DocumentSelector, InitializeParams, MessageType,
            Url,
        },
        Request,
    },
    register_plugin, Http, LapcePlugin, VoltEnvironment, PLUGIN_RPC,
};
use serde_json::Value;
use std::{fs::File, path::PathBuf};

#[derive(Default)]
struct State {}

register_plugin!(State);

fn initialize(params: InitializeParams) -> Result<()> {
    let document_selector: DocumentSelector = vec![DocumentFilter {
        // lsp language id
        language: Some(String::from("markdown")),
        // glob pattern
        pattern: Some(String::from("**/*.md")),
        // like file:
        scheme: None,
    }];
    let server_args = vec![];

    let server_path = params
        .initialization_options
        .as_ref()
        .and_then(|options| options.get("serverPath"))
        .and_then(|server_path| server_path.as_str())
        .and_then(|server_path| {
            if !server_path.is_empty() {
                Some(server_path)
            } else {
                None
            }
        });

    if let Some(server_path) = server_path {
        let program = match std::env::var("VOLT_OS").as_deref() {
            Ok("windows") => "where",
            _ => "which",
        };
        let exists = PLUGIN_RPC
            .execute_process(program.to_string(), vec![server_path.to_string()])
            .map(|r| r.success)
            .unwrap_or(false);
        if !exists {
            PLUGIN_RPC.window_show_message(
                MessageType::ERROR,
                format!("server path {server_path} couldn't be found, please check"),
            );
            return Ok(());
        }
        PLUGIN_RPC.start_lsp(
            Url::parse(&format!("urn:{server_path}"))?,
            server_args,
            document_selector,
            params.initialization_options,
        );
        return Ok(());
    }

    // Architecture check
    let arch = match VoltEnvironment::architecture().as_deref() {
        Ok("x86_64") => "marksman-linux-x64",
        Ok("aarch64") => "marksman-linux-arm64",
        _ => return Ok(()),
    };

    // OS check
    let file_name = match VoltEnvironment::operating_system().as_deref() {
        Ok("macos") => "marksman-macos",
        Ok("linux") => arch,
        Ok("windows") => "marksman.exe",
        _ => return Ok(()),
    };

    let file_path = PathBuf::from(&file_name);
    if !file_path.exists() {
        let result: Result<()> = {
            let url = format!(
                "https://github.com/artempyanykh/marksman/releases/download/2023-06-01/{file_name}"
            );
            let mut resp = Http::get(&url)?;
            let body = resp.body_read_all()?;
            let mut file = File::create(&file_path)?;
            std::io::copy(&mut body.as_slice(), &mut file)?;
            Ok(())
        };
        if result.is_err() {
            PLUGIN_RPC.window_show_message(
                MessageType::ERROR,
                "Unable to download Marksman, please use server path in the settings.".to_string(),
            );
            return Ok(());
        }
    }

    // Plugin working directory
    let volt_uri = VoltEnvironment::uri()?;
    let server_uri = Url::parse(&volt_uri)?.join(file_name)?;

    // Available language IDs
    // https://github.com/lapce/lapce/blob/HEAD/lapce-proxy/src/buffer.rs#L173
    PLUGIN_RPC.start_lsp(
        server_uri,
        server_args,
        document_selector,
        params.initialization_options,
    );

    Ok(())
}

impl LapcePlugin for State {
    fn handle_request(&mut self, _id: u64, method: String, params: Value) {
        #[allow(clippy::single_match)]
        match method.as_str() {
            Initialize::METHOD => {
                let params: InitializeParams = serde_json::from_value(params).unwrap();
                if let Err(e) = initialize(params) {
                    PLUGIN_RPC.window_show_message(
                        MessageType::ERROR,
                        format!("plugin returned with error: {e}"),
                    )
                }
            }
            _ => {}
        }
    }
}

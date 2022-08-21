use super::extension::main_extension;
use super::module::TypescriptModuleLoader;
use deno_core::error::AnyError;
use deno_core::{ModuleSpecifier, Extension};
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::{MainWorker, WorkerOptions};
use deno_runtime::BootstrapOptions;
use futures::channel::mpsc::UnboundedSender;
use std::{rc::Rc, sync::Arc};

// https://github.com/denoland/deno/blob/main/runtime/examples/hello_runtime.rs

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

fn options(extensions: Vec<Extension>) -> WorkerOptions {
    let module_loader = Rc::new(TypescriptModuleLoader);
    let create_web_worker_cb = Arc::new(|_| {
        todo!("Web workers are not supported within electrum");
    });
    let web_worker_preload_module_cb = Arc::new(|_| {
        todo!("Web workers are not supported within electrum");
    });

    WorkerOptions {
        bootstrap: BootstrapOptions {
            args: vec![],
            cpu_count: num_cpus::get(),
            debug_flag: false,
            enable_testing_features: false,
            location: None,
            no_color: false,
            is_tty: false,
            runtime_version: "x".to_string(),
            ts_version: "x".to_string(),
            unstable: false,
            user_agent: "electrum".to_string(),
        },
        extensions,
        unsafely_ignore_certificate_errors: None,
        root_cert_store: None,
        seed: None,
        source_map_getter: None,
        format_js_error_fn: None,
        web_worker_preload_module_cb,
        create_web_worker_cb,
        maybe_inspector_server: None,
        should_break_on_first_statement: false,
        module_loader,
        get_error_class_fn: Some(&get_error_class_name),
        origin_storage_dir: None,
        blob_store: BlobStore::default(),
        broadcast_channel: InMemoryBroadcastChannel::default(),
        shared_array_buffer_store: None,
        compiled_wasm_module_store: None,
        stdio: Default::default(),
    }
}

pub struct MainWorkerInstance {
    pub worker: MainWorker,
    pub event_sender: UnboundedSender<super::extension::Event>
}

pub fn new(main_module_path: ModuleSpecifier) -> MainWorkerInstance {
    let extension_instance = main_extension();
    let worker = MainWorker::bootstrap_from_options(main_module_path, Permissions::allow_all(), options(vec![extension_instance.extension]));

    MainWorkerInstance {
        worker,
        event_sender: extension_instance.event_sender
    }
}

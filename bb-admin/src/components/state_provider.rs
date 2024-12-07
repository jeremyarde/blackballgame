pub mod state_provider {
    use dioxus::prelude::*;
    use gloo_storage::{LocalStorage, Storage};
    use serde_json::Value;
    use svg_attributes::local;
    use tracing::info;

    use crate::{
        environment, server_client::server_client::ServerClient, AppProps, Env, Home, ServerConfig,
        UserConfig, USERNAME_KEY,
    };

    fn get_string_from_local_storage(key: &str) -> String {
        Value::String(
            LocalStorage::get(key)
                .unwrap_or(Value::String("".to_string()).as_str().unwrap().to_string()),
        )
        .as_str()
        .unwrap()
        .to_string()
    }

    #[component]
    pub(crate) fn StateProvider() -> Element {
        let environment = environment::get_env_variable();
        let is_debug = environment::get_debug_variable();

        info!("Environment: {:?}", environment);
        let is_prod = if environment.is_some() {
            environment.unwrap() == "production"
        } else {
            true // default to production
        };

        let mut app_props = use_context_provider(|| {
            Signal::new(AppProps {
                environment: if is_prod {
                    Env::Production
                } else {
                    Env::Development
                },
                debug_mode: is_debug.unwrap_or(false),
            })
        });

        let localstorage: String =
            use_context_provider(|| get_string_from_local_storage(USERNAME_KEY));

        info!("jere/ localstorage: {:?}", localstorage);
        let mut current_route = use_context_provider(|| Signal::new("Home".to_string()));
        let mut user_config = use_context_provider(|| {
            Signal::new(UserConfig {
                username: if is_prod {
                    localstorage
                } else {
                    String::from("player2")
                },
                lobby_code: String::new(),
                client_secret: String::new(),
            })
        });

        let mut server_config = use_context_provider(|| Signal::new(ServerConfig::new(is_prod)));
        let mut server_client = use_context_provider(|| {
            Signal::new(ServerClient::new(server_config.read().server_url.clone()))
        });
        // let mut websocket_connection = use_context_provider(|| Signal::new(None));

        rsx!(Home {})
    }
}

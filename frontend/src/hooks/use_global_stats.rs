use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::models::GlobalStats;
use crate::config::get_api_base_url;

#[hook]
pub fn use_global_stats() -> (bool, Option<GlobalStats>) {
    let stats = use_state(|| None::<GlobalStats>);
    let loading = use_state(|| true);

    {
        let stats = stats.clone();
        let loading = loading.clone();

        use_effect_with((), move |_| {
            loading.set(true);
            
            spawn_local(async move {
                match Request::get(&format!("{}/api/stats/global", get_api_base_url()))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status() == 200 {
                            if let Ok(data) = response.json::<GlobalStats>().await {
                                stats.set(Some(data));
                            }
                        }
                        loading.set(false);
                    }
                    Err(_) => {
                        loading.set(false);
                    }
                }
            });

            || ()
        });
    }

    (*loading, (*stats).clone())
} 
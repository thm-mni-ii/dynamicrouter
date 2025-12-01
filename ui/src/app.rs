use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Model {
    id: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ModelResponse {
    data: Vec<Model>,
}

async fn load_models(api_key: String) -> Option<Result<Vec<Model>, reqwest::Error>> {
    match api_key.as_str() {
        "" => None,
        _ => Some(
            match reqwest::Client::new()
                .get(format!("{}/v1/models", location().origin().unwrap()))
                .bearer_auth(api_key)
                .send()
                .await
            {
                Ok(res) => match res.error_for_status() {
                    Ok(res) => match res.json::<ModelResponse>().await {
                        Ok(models) => Ok(models.data),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            },
        ),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Clone, Debug, Serialize)]
struct Request {
    model: String,
    messages: Vec<Message>,
}

#[derive(Clone, Debug, Deserialize)]
struct MessageChoice {
    message: Message,
}

#[derive(Clone, Debug, Deserialize)]
struct Response {
    choices: Vec<MessageChoice>,
}

async fn generate(
    api_key: String,
    request: Option<Request>,
) -> Option<Result<Response, reqwest::Error>> {
    println!("{:?}", request);
    match request {
        None => None,
        Some(request) => Some(
            match reqwest::Client::new()
                .post(format!(
                    "{}/v1/chat/completions",
                    location().origin().unwrap()
                ))
                .bearer_auth(api_key)
                .json(&request)
                .send()
                .await
            {
                Ok(res) => match res.error_for_status() {
                    Ok(res) => res.json().await,
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            },
        ),
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (api_key, set_api_key) = signal("".to_string());
    let (model, set_model) = signal("".to_string());
    let (system_prompt, set_system_prompt) = signal("".to_string());
    let (user_input, set_user_input) = signal("".to_string());
    let (request, set_request) = signal::<Option<Request>>(None);
    let (submit_disabled, set_submit_disabled) = signal(false);

    let models = LocalResource::new(move || load_models(api_key.get()));
    let response = LocalResource::new(move || generate(api_key.get(), request.get()));

    Effect::new(move || *set_submit_disabled.write() = (*response.read()).is_none());

    let do_request = move |_| {
        *set_request.write() = Some(Request {
            model: model.get(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt.get(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_input.get(),
                },
            ],
        });
        *set_submit_disabled.write() = true;
    };

    view! {
        <main class="container">
            <fieldset>
                <label>
                    API Key
                    <input
                        name="api_key"
                        placeholder="API Key"
                        type="password"
                        bind:value=(api_key, set_api_key)
                    />
                </label>
                {move || match &*models.read() {
                    Some(Some(result)) => match result {
                        Ok(models) => view! {<label>
                            Model
                            <select
                                name="model"
                                bind:value=(model, set_model)
                            >
                                {models.iter()
                                    .map(|Model {id, name}| view! {<option value={id.clone()}>{name.clone()}</option>})
                                    .collect::<Vec<_>>()}
                            </select>
                        </label>}.into_any(),
                        Err(e) => view! {{format!("{:?}", e)}}.into_any()
                    },
                    _ => ().into_any(),
                }}
                {move || match model.get().as_str() {
                    "" => ().into_any(),
                    _ => view! {
                    <label>
                    System Prompt
                    <textarea
                        name="system_prompt"
                        placeholder="System Prompt"
                        bind:value=(system_prompt, set_system_prompt)
                    />
                </label>
                <label>
                    User Input
                    <textarea
                        name="user_input"
                        placeholder="User Input"
                        bind:value=(user_input, set_user_input)
                    />
                </label>
                <button disabled=submit_disabled on:click=do_request>
                    Submit
                </button>
                    }.into_any(),
                }}
            </fieldset>
            {move || match &*response.read() {
                Some(Some(res)) => match res {
                    Ok(response) => view! {<pre>{response.choices[0].message.content.clone()}</pre>}.into_any(),
                    Err(e) => view! {{format!("{:?}", e)}}.into_any()
                },
                _ => ().into_any()
            }}
        </main>
    }
}

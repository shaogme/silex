use silex_core::{ErrorHandlerToken, OwnerAccess, Runtime, SilexError, SilexResult};
use silex_net::{CredentialsMode, HttpClient, HttpMethod, RequestBody, RequestSpec, RetryPolicy};
use std::{error::Error, time::Duration};

fn handler<'owner>(owner: OwnerAccess<'owner>) -> SilexResult<ErrorHandlerToken<'owner>> {
    owner.error_handler(|_| {})
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let builder = HttpClient::get(
                owner,
                "https://api.example.test/items/{id}",
                handler(owner)?,
            )
            .path_param("id", 7_i32)
            .query("limit", 20_i32)
            .header("Accept", "text/plain")
            .timeout(Duration::from_secs(5));

            // `send` 在浏览器中由调用方 await；这里仅验证 owner-bound builder 的
            // 构造和 future 类型，不触碰 native 环境不存在的 browser host。
            let request = builder.send();
            drop(request);

            let spec = RequestSpec {
                method: HttpMethod::Get,
                url: "https://api.example.test/items/7?limit=20".to_string(),
                headers: vec![("Accept".to_string(), "text/plain".to_string())],
                credentials: CredentialsMode::Omit,
                timeout: Some(Duration::from_secs(5)),
                body: RequestBody::Empty,
            };
            assert!(spec.is_persistent_cache_safe());
            assert_eq!(spec.method.as_str(), "GET");

            let retry = RetryPolicy::new(2, Duration::from_millis(100))
                .max_delay(Duration::from_millis(500))
                .no_jitter();
            assert_eq!(retry.delay_for_attempt(1), Duration::from_millis(100));
            assert_eq!(retry.delay_for_attempt(2), Duration::from_millis(200));
            Ok::<(), SilexError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}

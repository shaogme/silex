use silex_net::HttpClient;

fn build<'scope>(scope: silex_core::Scope<'scope>) {
        let (post_id, _) = scope.signal(1_i32);
        let query = scope.rw_signal(String::new());
        let token = scope.rw_signal("token".to_string());
        let resource = HttpClient::get(
            scope,
            "https://example.test/posts/{id}",
            scope.error_handler(|_| {}),
        )
            .path_param("id", post_id)
            .query("filter", query)
            .header("Authorization", token)
            .as_resource(post_id, None);
        let _ = resource.state.get();
}

fn main() {
}

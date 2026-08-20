use silex_net::HttpClient;

fn build<'scope>(scope: silex_core::OwnerAccess<'scope>) {
        let (post_id, _) = scope.signal(1_i32).unwrap();
        let query = scope.rw_signal(String::new()).unwrap();
        let token = scope.rw_signal("token".to_string()).unwrap();
        let resource = HttpClient::get(
            scope,
            "https://example.test/posts/{id}",
            scope.error_handler(|_| {}).unwrap(),
        )
            .path_param("id", post_id)
            .query("filter", query)
            .header("Authorization", token)
            .as_resource(post_id, None)
            .unwrap();
        let _ = resource.state().get().unwrap();
}

fn main() {
}

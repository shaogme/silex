use silex_router::macros::router;

router! {
    enum TenantRoute {
        Home => "/",
    }
}

router! {
    enum DynamicPrefix {
        Tenant(TenantRoute) {
            prefix: "/:tenant";
            layout: |_ctx, outlet| outlet;
        },
    }
}

fn main() {}

use silex_router::macros::router;

router! {
    enum DynamicPrefix {
        Tenant {
            prefix: "/:tenant";
            layout: |_context, outlet| outlet;
            children: { Home => "/" }
        },
    }
}

fn main() {}

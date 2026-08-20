use silex_i18n::CatalogResource;

fn assert_copy<T: Copy>() {}

fn main() {
    assert_copy::<CatalogResource<'static, String>>();
}

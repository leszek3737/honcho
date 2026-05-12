use static_assertions::assert_impl_all;
use honcho_ai::http::client::HttpClient;

assert_impl_all!(HttpClient: Send, Sync, Clone);

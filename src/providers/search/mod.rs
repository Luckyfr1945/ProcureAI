pub mod provider_adapter;
pub mod traits;

pub use provider_adapter::{create_search_provider, HttpSearchProvider, MockSearchProvider};
pub use traits::{SearchItem, SearchPage, SearchProvider};


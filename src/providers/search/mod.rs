pub mod provider_adapter;
pub mod traits;

pub use provider_adapter::{HttpSearchProvider, MockSearchProvider, create_search_provider};
pub use traits::{SearchItem, SearchPage, SearchProvider};

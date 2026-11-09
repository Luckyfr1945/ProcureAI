use crate::providers::gemini::{ExpandedQueryOutput, GeminiClient};

pub struct QueryExpansionService {
    gemini: GeminiClient,
}

impl QueryExpansionService {
    pub fn new(gemini: GeminiClient) -> Self {
        Self { gemini }
    }

    pub async fn expand(
        &self,
        request: &str,
        max_queries: usize,
    ) -> anyhow::Result<ExpandedQueryOutput> {
        self.gemini.expand_search_query(request, max_queries).await
    }
}

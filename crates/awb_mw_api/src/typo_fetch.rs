use crate::error::MwApiError;
use url::Url;

/// Fetch typo-fix rules from a Wikipedia page (typically Wikipedia:AutoWikiBrowser/Typos)
///
/// This function fetches the raw wikitext of a page containing typo-fix rules.
/// The returned text can be parsed by `awb_engine::typo_fix::TypoFixer::from_str()`.
///
/// # Arguments
/// * `client` - HTTP client to use for the request
/// * `api_url` - MediaWiki API URL
/// * `page_title` - Title of the page containing typo rules (e.g., "Wikipedia:AutoWikiBrowser/Typos")
///
/// # Returns
/// Raw wikitext content of the page
///
/// # Example
/// ```no_run
/// # use awb_mw_api::typo_fetch::fetch_typo_fix_rules;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = reqwest::Client::new();
/// let api_url = url::Url::parse("https://en.wikipedia.org/w/api.php")?;
/// let rules_text = fetch_typo_fix_rules(
///     &client,
///     &api_url,
///     "Wikipedia:AutoWikiBrowser/Typos"
/// ).await?;
/// // Now parse with TypoFixer::from_str(&rules_text)
/// # Ok(())
/// # }
/// ```
pub async fn fetch_typo_fix_rules(
    client: &reqwest::Client,
    api_url: &Url,
    page_title: &str,
) -> Result<String, MwApiError> {
    let params = [
        ("action", "query"),
        ("titles", page_title),
        ("prop", "revisions"),
        ("rvprop", "content"),
        ("rvslots", "main"),
        ("format", "json"),
    ];

    let resp: serde_json::Value = client
        .get(api_url.as_str())
        .query(&params)
        .send()
        .await?
        .json()
        .await?;

    // Check for API errors
    if let Some(error) = resp.get("error") {
        let code = error["code"].as_str().unwrap_or("unknown").to_string();
        let info = error["info"].as_str().unwrap_or("").to_string();
        return Err(MwApiError::ApiError { code, info });
    }

    // Parse response
    let pages = &resp["query"]["pages"];
    let page = pages
        .as_object()
        .and_then(|m| m.values().next())
        .ok_or_else(|| MwApiError::ApiError {
            code: "nopage".into(),
            info: "No page data returned".into(),
        })?;

    // Check if page exists
    if page.get("missing").is_some() {
        return Err(MwApiError::ApiError {
            code: "missingtitle".into(),
            info: format!("Page '{}' does not exist", page_title),
        });
    }

    // Extract wikitext
    let wikitext = page["revisions"][0]["slots"]["main"]["content"]
        .as_str()
        .ok_or_else(|| MwApiError::ApiError {
            code: "nocontent".into(),
            info: "No content in page revision".into(),
        })?
        .to_string();

    Ok(wikitext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_fetch_typo_fix_rules_success() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "query": {
                "pages": {
                    "12345": {
                        "pageid": 12345,
                        "title": "Wikipedia:AutoWikiBrowser/Typos",
                        "revisions": [{
                            "slots": {
                                "main": {
                                    "content": "<Typo find=\"\\bcolour\\b\" replace=\"color\" />"
                                }
                            }
                        }]
                    }
                }
            }
        });

        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .and(query_param("titles", "Wikipedia:AutoWikiBrowser/Typos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let api_url = Url::parse(&mock_server.uri()).unwrap();

        let result =
            fetch_typo_fix_rules(&client, &api_url, "Wikipedia:AutoWikiBrowser/Typos").await;

        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Typo"));
        assert!(content.contains("colour"));
    }

    #[tokio::test]
    async fn test_fetch_typo_fix_rules_missing_page() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "query": {
                "pages": {
                    "-1": {
                        "missing": true,
                        "title": "Wikipedia:AutoWikiBrowser/NonExistent"
                    }
                }
            }
        });

        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let api_url = Url::parse(&mock_server.uri()).unwrap();

        let result = fetch_typo_fix_rules(
            &client,
            &api_url,
            "Wikipedia:AutoWikiBrowser/NonExistent",
        )
        .await;

        assert!(result.is_err());
        match result {
            Err(MwApiError::ApiError { code, .. }) => {
                assert_eq!(code, "missingtitle");
            }
            _ => panic!("Expected ApiError with missingtitle"),
        }
    }

    #[tokio::test]
    async fn test_fetch_typo_fix_rules_api_error() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "error": {
                "code": "readapidenied",
                "info": "You need read permission to use this module"
            }
        });

        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let api_url = Url::parse(&mock_server.uri()).unwrap();

        let result =
            fetch_typo_fix_rules(&client, &api_url, "Wikipedia:AutoWikiBrowser/Typos").await;

        assert!(result.is_err());
        match result {
            Err(MwApiError::ApiError { code, .. }) => {
                assert_eq!(code, "readapidenied");
            }
            _ => panic!("Expected ApiError"),
        }
    }

    #[tokio::test]
    async fn test_fetch_typo_fix_rules_tsv_format() {
        let mock_server = MockServer::start().await;

        let mock_response = serde_json::json!({
            "query": {
                "pages": {
                    "12345": {
                        "pageid": 12345,
                        "title": "Wikipedia:AutoWikiBrowser/Typos",
                        "revisions": [{
                            "slots": {
                                "main": {
                                    "content": "\\bcolour\\b\tcolor\n\\bcentre\\b\tcenter"
                                }
                            }
                        }]
                    }
                }
            }
        });

        Mock::given(method("GET"))
            .and(query_param("action", "query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let api_url = Url::parse(&mock_server.uri()).unwrap();

        let result =
            fetch_typo_fix_rules(&client, &api_url, "Wikipedia:AutoWikiBrowser/Typos").await;

        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("colour"));
        assert!(content.contains("color"));
        assert!(content.contains("centre"));
        assert!(content.contains("center"));
    }
}

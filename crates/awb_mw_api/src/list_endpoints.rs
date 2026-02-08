use crate::error::MwApiError;
use awb_domain::types::*;

/// Parse a standard MediaWiki query list response into Titles
pub fn parse_list_response(resp: &serde_json::Value, list_key: &str) -> Vec<Title> {
    resp["query"][list_key]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let ns = Namespace(item["ns"].as_i64()? as i32);
                    let name = item["title"].as_str()?.to_string();
                    Some(Title {
                        namespace: ns,
                        name: name.clone(),
                        display: name,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Get the continuation token from a response
pub fn get_continue_token(resp: &serde_json::Value, key: &str) -> Option<String> {
    resp["continue"][key].as_str().map(String::from)
}

/// Fetch all pages from a list endpoint using continuation
pub async fn fetch_all_pages(
    client: &reqwest::Client,
    api_url: &url::Url,
    base_params: &[(&str, &str)],
    list_key: &str,
    continue_key: &str,
) -> Result<Vec<Title>, MwApiError> {
    let mut all_titles = Vec::new();
    let mut continue_token: Option<String> = None;

    loop {
        let mut params: Vec<(&str, String)> = base_params
            .iter()
            .map(|(k, v)| (*k, v.to_string()))
            .collect();
        params.push(("format", "json".to_string()));
        if let Some(ref token) = continue_token {
            params.push((continue_key, token.clone()));
            params.push(("continue", "-||".to_string()));
        }

        let query_params: Vec<(&str, &str)> =
            params.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp: serde_json::Value = client
            .get(api_url.as_str())
            .query(&query_params)
            .send()
            .await?
            .json()
            .await?;

        // Check for errors
        if let Some(error) = resp.get("error") {
            let code = error["code"].as_str().unwrap_or("unknown").to_string();
            let info = error["info"].as_str().unwrap_or("").to_string();
            return Err(MwApiError::ApiError { code, info });
        }

        let titles = parse_list_response(&resp, list_key);
        all_titles.extend(titles);

        match get_continue_token(&resp, continue_key) {
            Some(token) => continue_token = Some(token),
            None => break,
        }
    }

    Ok(all_titles)
}

/// Fetch pages from the user's watchlist
///
/// # Arguments
/// * `client` - HTTP client to use for the request
/// * `api_url` - MediaWiki API URL
/// * `limit` - Maximum number of pages to fetch (0 = unlimited)
///
/// # Returns
/// Vector of page titles from the watchlist
///
/// # Example
/// ```no_run
/// # use awb_mw_api::list_endpoints::fetch_watchlist;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = reqwest::Client::new();
/// let api_url = url::Url::parse("https://en.wikipedia.org/w/api.php")?;
/// let titles = fetch_watchlist(&client, &api_url, 100).await?;
/// # Ok(())
/// # }
/// ```
pub async fn fetch_watchlist(
    client: &reqwest::Client,
    api_url: &url::Url,
    limit: u32,
) -> Result<Vec<Title>, MwApiError> {
    let limit_str = if limit > 0 {
        limit.min(500).to_string()
    } else {
        "500".to_string()
    };

    let base_params = [
        ("action", "query"),
        ("list", "watchlistraw"),
        ("wrlimit", &limit_str),
    ];

    let mut titles =
        fetch_all_pages(client, api_url, &base_params, "watchlistraw", "wrcontinue").await?;

    if limit > 0 && titles.len() > limit as usize {
        titles.truncate(limit as usize);
    }

    Ok(titles)
}

/// Fetch pages from a user's contributions
///
/// # Arguments
/// * `client` - HTTP client to use for the request
/// * `api_url` - MediaWiki API URL
/// * `username` - Username to fetch contributions for
/// * `limit` - Maximum number of pages to fetch (0 = unlimited)
///
/// # Returns
/// Vector of page titles the user has contributed to
///
/// # Example
/// ```no_run
/// # use awb_mw_api::list_endpoints::fetch_user_contributions;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = reqwest::Client::new();
/// let api_url = url::Url::parse("https://en.wikipedia.org/w/api.php")?;
/// let titles = fetch_user_contributions(&client, &api_url, "ExampleUser", 100).await?;
/// # Ok(())
/// # }
/// ```
pub async fn fetch_user_contributions(
    client: &reqwest::Client,
    api_url: &url::Url,
    username: &str,
    limit: u32,
) -> Result<Vec<Title>, MwApiError> {
    let limit_str = if limit > 0 {
        limit.min(500).to_string()
    } else {
        "500".to_string()
    };

    let base_params = [
        ("action", "query"),
        ("list", "usercontribs"),
        ("ucuser", username),
        ("uclimit", &limit_str),
    ];

    let mut titles =
        fetch_all_pages(client, api_url, &base_params, "usercontribs", "uccontinue").await?;

    if limit > 0 && titles.len() > limit as usize {
        titles.truncate(limit as usize);
    }

    Ok(titles)
}

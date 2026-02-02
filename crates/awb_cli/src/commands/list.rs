use anyhow::{Context, Result};
use awb_domain::types::Title;
use awb_mw_api::list_endpoints::{fetch_all_pages, fetch_user_contributions, fetch_watchlist};
use console::style;
use url::Url;

use crate::ListSource;

pub async fn run(wiki: Url, source: ListSource, query: String, limit: usize) -> Result<()> {
    println!("{}", style("Fetching page list").bold().cyan());
    println!("Wiki: {}", wiki);
    println!("Source: {:?}", source);
    if !matches!(source, ListSource::Watchlist) {
        println!("Query: {}", query);
    }
    println!();

    let titles = match source {
        ListSource::Category => fetch_category_members(&wiki, &query, limit).await?,
        ListSource::WhatLinksHere => fetch_what_links_here(&wiki, &query, limit).await?,
        ListSource::Search => fetch_search_results(&wiki, &query, limit).await?,
        ListSource::File => fetch_from_file(&query).await?,
        ListSource::Watchlist => fetch_watchlist_pages(&wiki, limit).await?,
        ListSource::UserContribs => fetch_user_contribs(&wiki, &query, limit).await?,
    };

    println!(
        "{} Found {} pages:",
        style("✓").green().bold(),
        style(titles.len()).yellow().bold()
    );
    println!();

    for (i, title) in titles.iter().enumerate().take(limit.max(1)) {
        println!("  {}. {}", style(i + 1).dim(), title.display);
    }

    if titles.len() > limit && limit > 0 {
        println!();
        println!(
            "  {} (showing first {} of {} total)",
            style("...").dim(),
            limit,
            titles.len()
        );
    }

    Ok(())
}

async fn fetch_category_members(api_url: &Url, category: &str, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::builder()
        .user_agent("AWB-RS/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let category_title = if category.starts_with("Category:") {
        category.to_string()
    } else {
        format!("Category:{}", category)
    };

    let base_params = [
        ("action", "query"),
        ("list", "categorymembers"),
        ("cmtitle", &category_title),
        ("cmlimit", "500"),
    ];

    let mut titles = fetch_all_pages(
        &client,
        api_url,
        &base_params,
        "categorymembers",
        "cmcontinue",
    )
    .await
    .context("Failed to fetch category members")?;

    if limit > 0 && titles.len() > limit {
        titles.truncate(limit);
    }

    Ok(titles)
}

async fn fetch_what_links_here(api_url: &Url, page: &str, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::builder()
        .user_agent("AWB-RS/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let base_params = [
        ("action", "query"),
        ("list", "backlinks"),
        ("bltitle", page),
        ("bllimit", "500"),
    ];

    let mut titles = fetch_all_pages(&client, api_url, &base_params, "backlinks", "blcontinue")
        .await
        .context("Failed to fetch backlinks")?;

    if limit > 0 && titles.len() > limit {
        titles.truncate(limit);
    }

    Ok(titles)
}

async fn fetch_search_results(
    api_url: &Url,
    search_query: &str,
    limit: usize,
) -> Result<Vec<Title>> {
    let client = reqwest::Client::builder()
        .user_agent("AWB-RS/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let base_params = [
        ("action", "query"),
        ("list", "search"),
        ("srsearch", search_query),
        ("srlimit", "500"),
    ];

    let mut titles = fetch_all_pages(&client, api_url, &base_params, "search", "sroffset")
        .await
        .context("Failed to fetch search results")?;

    if limit > 0 && titles.len() > limit {
        titles.truncate(limit);
    }

    Ok(titles)
}

async fn fetch_from_file(file_path: &str) -> Result<Vec<Title>> {
    // Verify file exists and is a regular file (not a symlink)
    let metadata = tokio::fs::metadata(file_path)
        .await
        .context("Failed to access file")?;

    if !metadata.is_file() {
        anyhow::bail!("Path is not a regular file");
    }

    // Check file size (reject files >10MB)
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
    if metadata.len() > MAX_FILE_SIZE {
        anyhow::bail!("File too large (max 10MB)");
    }

    let content = tokio::fs::read_to_string(file_path)
        .await
        .context("Failed to read file")?;

    let titles: Vec<Title> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim();
            awb_domain::types::Title::new(awb_domain::types::Namespace::MAIN, trimmed)
        })
        .collect();

    Ok(titles)
}

async fn fetch_watchlist_pages(api_url: &Url, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::builder()
        .user_agent("AWB-RS/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let titles = fetch_watchlist(&client, api_url, limit as u32)
        .await
        .context("Failed to fetch watchlist")?;

    Ok(titles)
}

async fn fetch_user_contribs(api_url: &Url, username: &str, limit: usize) -> Result<Vec<Title>> {
    let client = reqwest::Client::builder()
        .user_agent("AWB-RS/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let titles = fetch_user_contributions(&client, api_url, username, limit as u32)
        .await
        .context("Failed to fetch user contributions")?;

    Ok(titles)
}

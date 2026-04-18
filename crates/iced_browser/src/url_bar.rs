const SEARCH_URL: &str = "https://duckduckgo.com/?q=%s";

/// Parse URL bar input into a navigable URL, following servoshell's
/// fallback chain: direct URL → file path → domain → search query.
pub fn input_to_url(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    url::Url::parse(input)
        .ok()
        .or_else(|| try_as_file(input))
        .or_else(|| try_as_domain(input))
        .or_else(|| try_as_search(input))
        .map(|u| u.to_string())
}

fn try_as_file(input: &str) -> Option<url::Url> {
    let path = std::path::Path::new(input);
    if path.is_absolute() {
        return url::Url::from_file_path(path).ok();
    }
    None
}

fn try_as_domain(input: &str) -> Option<url::Url> {
    if input.contains(' ') {
        return None;
    }
    let has_dot_segments = !input.starts_with('.') && input.split('.').count() > 1;
    let has_path = !input.starts_with('/') && input.contains('/');
    if has_dot_segments || has_path {
        return url::Url::parse(&format!("https://{input}")).ok();
    }
    None
}

fn try_as_search(input: &str) -> Option<url::Url> {
    url::Url::parse(&SEARCH_URL.replace("%s", input)).ok()
}

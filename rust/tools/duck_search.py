import urllib.parse
import requests


def duckduckgo_search(query: str, max_results: int = 5) -> list[str]:
    """
    Perform a web search using DuckDuckGo's HTML endpoint and return results.

    Args:
        query: Search query string.
        max_results: Maximum number of results to return (default 5).

    Returns:
        A list of dicts, each containing:
            - 'title': result title
            - 'url': result URL
            - 'snippet': description/snippet text (may be empty)
    """
    if not query or not query.strip():
        raise ValueError("Query must be a non-empty string")

    url = "https://html.duckduckgo.com/html/"
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
        )
    }
    params = {"q": query}

    try:
        resp = requests.post(url, data=params, headers=headers,
!
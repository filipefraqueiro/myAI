"""
Web search tool using DuckDuckGo.

Uses DuckDuckGo's public HTML endpoint (https://html.duckduckgo.com/html/)
which does not require an API key. Results are parsed from the HTML response.
"""

import re
from typing import Dict, List, Any
from urllib.parse import urlencode, unquote, quote_plus

import requests


def _parse_results(html: str) -> List[Dict[str, str]]:
    """Parse search results out of DuckDuckGo HTML response."""
    results = []
    # Each result block looks like:
    # <a rel="nofollow" class="result__a" href="...">Title</a>
    # ... <a class="result__snippet" ...>Snippet</a>


    # Simpler robust regex approach
    result_pattern = re.compile(
        r'class="result__a"[^>]*href="([^"]+)"[^>]*>(.*?)</a>'
        r'.*?class="result__snippet"[^>]*>(.*?)</a>',
        re.DOTALL,
    )

    for match in result_pattern.finditer(html):
        url, title, snippet = match.groups()
        # DDG wraps external URLs in a redirect like //duck.d
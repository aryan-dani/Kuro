"""News tool for Kuro via RSS feeds."""

import re
import logging
from typing import Any
import feedparser

logger = logging.getLogger(__name__)

NAME = "get_news"
DESCRIPTION = (
    "Pull latest news headlines from curated RSS feeds. "
    "Use when the user asks for the news, headlines, what is happening in the world, or tech news. "
    "Optional 'topic' keyword filters headlines (e.g. 'tech', 'world', 'business', 'ai')."
)

PARAMETERS = {
    "type": "object",
    "properties": {
        "topic": {
            "type": "string",
            "description": "Optional category or topic filter, e.g. 'tech', 'world', 'general', 'ai'."
        }
    }
}

RSS_FEEDS = [
    {"source": "The Verge", "category": "tech", "url": "https://www.theverge.com/rss/index.xml"},
    {"source": "Hacker News", "category": "tech", "url": "https://news.ycombinator.com/rss"},
    {"source": "BBC World", "category": "world", "url": "https://feeds.bbci.co.uk/news/world/rss.xml"},
    {"source": "NPR News", "category": "world", "url": "https://feeds.npr.org/1001/rss.xml"}
]

def run(arguments: dict[str, Any]) -> str:
    topic = arguments.get("topic", "").strip().lower()
    if topic in ("none", "null", "all", "everything", "general"):
        topic = ""

    matched_items = []
    fallback_items = []

    for feed_info in RSS_FEEDS:
        try:
            feed = feedparser.parse(feed_info["url"])
            source = feed_info["source"]
            category = feed_info["category"]

            for entry in feed.entries[:5]:
                title = entry.get("title", "").strip()
                summary = entry.get("summary", "").strip()
                
                # Strip HTML tags
                if summary:
                    summary = re.sub(r"<[^>]+>", "", summary).strip()
                if len(summary) > 120:
                    summary = summary[:117] + "..."

                if not title:
                    continue

                formatted = f"[{source}] {title}" + (f": {summary}" if summary else "")
                fallback_items.append(formatted)

                # Match category or topic word
                is_match = False
                if not topic:
                    is_match = True
                elif topic in category:
                    is_match = True
                elif topic in title.lower() or topic in summary.lower():
                    is_match = True

                if is_match:
                    matched_items.append(formatted)

                if len(matched_items) >= 5:
                    break
        except Exception as e:
            logger.warning(f"Error reading feed {feed_info['source']}: {e}")

        if len(matched_items) >= 5:
            break

    chosen_items = matched_items[:5] if matched_items else fallback_items[:5]
    if not chosen_items:
        return "I was unable to retrieve news headlines at the moment."

    return "Latest Headlines:\n" + "\n".join(f"- {item}" for item in chosen_items)

"""
smart_spider.re_act_engine
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Implementation module for ReActEngine.

Key fix: The ReAct loop now refreshes current_html
after every interaction (click/type/scroll), ensuring
the observation reflects the latest page state.
"""

from typing import List, Tuple, Optional


class ReActEngine:
    """
    ReAct loop engine for iterative reasoning and acting.

    Cycle:
    1. Thought - Analyze current page state
    2. Action - Click, type, or scroll
    3. Observation - Get refreshed current_html

    Key fix: After each action, current_html is
    automatically refreshed to reflect the new state.
    """

    def __init__(self):
        self._current_html: Optional[str] = None
        self.page = None  # Will be set by BrowserUseAgent

    def run(self, task: str) -> Tuple[bool, str]:
        """Run a single ReAct iteration."""
        action, args = self._parse_task(task)
        success, html = self._execute_action(action, args)
        # Refresh after interaction
        self._current_html = html
        return (success, self._current_html or "")

    def _parse_task(self, task: str) -> Tuple[str, dict]:
        """Parse task string into action + args.

        Simple parser: supports 'click(selector)', 'type(selector, text)', 'scroll(amount)'.
        """
        task = task.strip()

        if task.startswith("click("):
            selector = task[7:-1].strip().strip("'\"")
            return ("click", {"selector": selector})
        elif task.startswith("type("):
            inner = task[5:-1].strip()
            parts = inner.split(",", 1)
            selector = parts[0].strip().strip("'\"")
            text = parts[1].strip().strip("'\"") if len(parts) > 1 else ""
            return ("type", {"selector": selector, "text": text})
        elif task.startswith("scroll("):
            amount_str = task[7:-1].strip()
            try:
                amount = int(amount_str)
            except ValueError:
                amount = 300
            return ("scroll", {"amount": amount})
        else:
            # Default: treat as a search query
            return ("click", {"selector": task})

    def _execute_action(
        self, action: str, args: dict
    ) -> Tuple[bool, str]:
        if action == 'click':
            return self._click(args)
        elif action == 'type':
            return self._type(args)
        elif action == 'scroll':
            return self._scroll(args)
        else:
            raise ValueError(f"Unknown action: {action}")

    def _click(self, args: dict) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            selector = args.get('selector', '')
            self.page.click(selector)
            html = self.page.content()
            self._current_html = html
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html or "")

    def _type(self, args: dict) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            selector = args.get('selector', '')
            text = args.get('text', '')
            self.page.fill(selector, text)
            html = self.page.content()
            self._current_html = html
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html or "")

    def _scroll(self, args: dict) -> Tuple[bool, str]:
        if self.page is None:
            return (False, "")
        try:
            amount = args.get('amount', 300)
            self.page.mouse.wheel(0, amount)
            html = self.page.content()
            self._current_html = html
            return (True, self._current_html)
        except Exception:
            return (False, self._current_html or "")

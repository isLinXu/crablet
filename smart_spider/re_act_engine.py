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

    def run(self, task: str) -> Tuple[bool, str]:
        """Run a single ReAct iteration."""
        action, args = self._parse_task(task)
        success, html = self._execute_action(action, args)
        # Refresh after interaction
        self._current_html = html
        return (success, self._current_html)

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
        selector = args.get('selector', '')
        success = self.page.click(selector)
        html = self.page.content()
        self._current_html = html
        return (success, self._current_html)

    def _type(self, args: dict) -> Tuple[bool, str]:
        selector = args.get('selector', '')
        text = args.get('text', '')
        success = self.page.fill(selector, text)
        html = self.page.content()
        self._current_html = html
        return (success, self._current_html)

    def _scroll(self, args: dict) -> Tuple[bool, str]:
        amount = args.get('amount', 0)
        success = self.page.mouse.wheel(0, amount)
        html = self.page.content()
        self._current_html = html
        return (success, self._current_html)

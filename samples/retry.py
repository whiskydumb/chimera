"""retry decorator with exponential backoff."""
from __future__ import annotations

import functools
import time
from typing import Callable, TypeVar

T = TypeVar("T")


def retry(attempts: int = 3, base_delay: float = 0.5) -> Callable[[Callable[..., T]], Callable[..., T]]:
    """retry a callable up to `attempts` times, backing off exponentially."""

    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @functools.wraps(func)
        def wrapper(*args: object, **kwargs: object) -> T:
            for attempt in range(1, attempts + 1):
                try:
                    return func(*args, **kwargs)
                except Exception:
                    if attempt == attempts:
                        raise
                    time.sleep(base_delay * 2 ** (attempt - 1))
            raise RuntimeError("unreachable")

        return wrapper

    return decorator

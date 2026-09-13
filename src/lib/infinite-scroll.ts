export const LOAD_MORE_ROOT_MARGIN = "240px 0px";

export type ObserveLoadMoreOptions = {
  onLoad: () => void;
  enabled?: boolean;
  rootMargin?: string;
};

export type ObserveLoadMoreParam = ObserveLoadMoreOptions | (() => void);

type NormalizedObserveLoadMoreOptions = {
  onLoad: () => void;
  enabled: boolean;
  rootMargin: string;
};

export function loadMoreObserverEnabled(options: {
  enabled?: boolean;
  loading?: boolean;
  error?: string;
}): boolean {
  return Boolean(options.enabled) && !options.loading && !options.error;
}

export function normalizeObserveLoadMoreOptions(
  options: ObserveLoadMoreParam | null | undefined,
): NormalizedObserveLoadMoreOptions {
  if (typeof options === "function") {
    return {
      onLoad: options,
      enabled: true,
      rootMargin: LOAD_MORE_ROOT_MARGIN,
    };
  }

  return {
    onLoad: options?.onLoad ?? (() => {}),
    enabled: options?.enabled !== false && typeof options?.onLoad === "function",
    rootMargin: options?.rootMargin ?? LOAD_MORE_ROOT_MARGIN,
  };
}

export function observeLoadMore(node: HTMLElement, options?: ObserveLoadMoreParam) {
  let current = normalizeObserveLoadMoreOptions(options);
  let observer: IntersectionObserver | null = null;
  let fired = false;

  function onIntersect(entries: IntersectionObserverEntry[]) {
    if (!current.enabled || fired) return;
    if (!entries.some((entry) => entry.isIntersecting)) return;
    fired = true;
    current.onLoad();
  }

  function connect() {
    observer?.disconnect();
    observer = null;
    fired = false;
    if (typeof IntersectionObserver === "undefined" || !current.enabled) return;
    observer = new IntersectionObserver(onIntersect, {
      root: null,
      rootMargin: current.rootMargin,
    });
    observer.observe(node);
  }

  connect();

  return {
    update(next?: ObserveLoadMoreParam) {
      const previous = current;
      current = normalizeObserveLoadMoreOptions(next);
      if (
        previous.enabled !== current.enabled ||
        previous.rootMargin !== current.rootMargin ||
        !observer
      ) {
        connect();
        return;
      }
    },
    destroy() {
      observer?.disconnect();
      observer = null;
    },
  };
}

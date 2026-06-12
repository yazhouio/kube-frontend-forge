import {
  ColumnDef,
  ColumnFiltersState,
  SortingState,
  PaginationState,
  VisibilityState,
} from "@tanstack/react-table";
import { create, type StoreApi, type UseBoundStore } from "zustand";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { throttle } from "es-toolkit/compat";
import { lruCache } from "../../utils/cache";
// import { PageTableController } from '../useTanStackPageTable';

type Updater<T> = T | ((prev: T) => T);

type PageState = {
  query: Record<string, any>;
  table: {
    columnFilters: ColumnFiltersState;
    sorting: SortingState;
    columnVisibility: VisibilityState;
  };
  pagination: PaginationState;
};

export type PageStore = PageState & {
  setQuery: (
    updater:
      | Record<string, any>
      | ((prev: Record<string, any>) => Record<string, any>),
  ) => void;
  setColumnFilters: (updater: Updater<ColumnFiltersState>) => void;
  setSorting: (updater: Updater<SortingState>) => void;
  setColumnVisibility: (updater: Updater<VisibilityState>) => void;
  setPagination: (updater: Updater<PaginationState>) => void;
  reset: () => void;
};

/* =========================
 * Constants and helpers
 * ========================= */

const STORAGE_PREFIX = "page-store";

const getStorageKey = (pageId: string) => `${STORAGE_PREFIX}:${pageId}`;
const getColumnVisibilityStorageKey = (pageId: string) =>
  `${STORAGE_PREFIX}:column-visibility:${pageId}`;

function sanitizeColumnVisibility(
  raw: unknown,
  columnIdSet: Set<string>,
): VisibilityState {
  if (!raw || typeof raw !== "object") return {};

  const next: VisibilityState = {};
  Object.entries(raw as Record<string, unknown>).forEach(([key, value]) => {
    if (!columnIdSet.has(key)) return;
    if (typeof value !== "boolean") return;
    next[key] = value;
  });
  return next;
}

function isSameColumnVisibility(
  a: VisibilityState,
  b: VisibilityState,
): boolean {
  const aEntries = Object.entries(a);
  const bEntries = Object.entries(b);
  if (aEntries.length !== bEntries.length) return false;

  for (const [key, value] of aEntries) {
    if (b[key] !== value) return false;
  }
  return true;
}

function readColumnVisibility(
  pageId: string,
  columnIdSet: Set<string>,
): VisibilityState {
  if (typeof localStorage === "undefined") return {};

  try {
    const raw = localStorage.getItem(getColumnVisibilityStorageKey(pageId));
    if (!raw) return {};
    return sanitizeColumnVisibility(JSON.parse(raw), columnIdSet);
  } catch {
    return {};
  }
}

function writeColumnVisibility(
  pageId: string,
  visibility: VisibilityState,
): void {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.setItem(
      getColumnVisibilityStorageKey(pageId),
      JSON.stringify(visibility),
    );
  } catch {
    // Best-effort only.
  }
}

function clearColumnVisibility(pageId: string): void {
  if (typeof localStorage === "undefined") return;

  try {
    localStorage.removeItem(getColumnVisibilityStorageKey(pageId));
  } catch {
    // Best-effort only.
  }
}

function extractColumnIds<T>(columns: ColumnDef<T>[]): Set<string> {
  const ids = new Set<string>();

  const visit = (defs: Array<ColumnDef<any>>) => {
    defs?.forEach((def) => {
      const anyDef = def as any;
      if (Array.isArray(anyDef.columns)) {
        visit(anyDef.columns);
      }

      if (typeof anyDef.id === "string") {
        ids.add(anyDef.id);
        return;
      }
      if (typeof anyDef.accessorKey === "string") {
        ids.add(anyDef.accessorKey);
      }
    });
  };

  visit(columns as any);
  return ids;
}

function parseFilterValue(raw: string): unknown {
  let decoded = raw;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    // Best-effort only.
  }

  const trimmed = decoded.trimStart();
  const first = trimmed[0];
  if (first === "{" || first === "[" || first === '"') {
    try {
      return JSON.parse(decoded);
    } catch {
      return decoded;
    }
  }

  return decoded;
}

function shouldUseCache(search: string): boolean {
  const raw = new URLSearchParams(search).get("__cache__");
  if (!raw) return false;
  return ["true", "1", "yes"].includes(String(raw).toLowerCase());
}

function parseFromUrl(
  search: string,
  columnIdSet: Set<string>,
): PageState | null {
  if (!search) return null;

  const params = new URLSearchParams(search);
  if (params.size === 1 && params.get("__cache__") === "true") return null;

  const query: Record<string, any> = {};

  const columnFilterMap = new Map<string, unknown>();
  const filters = params.get("filters");
  if (filters) {
    filters.split(",").forEach((pair) => {
      const [id, ...rest] = pair.split(":");
      if (!id) return;
      const raw = rest.join(":");
      columnFilterMap.set(id, parseFilterValue(raw));
    });
  }

  params.forEach((v, k) => {
    if (
      [
        "filters",
        "sort",
        "sortBy",
        "ascending",
        "page",
        "pageSize",
        "limit",
        "__cache__",
      ].includes(k)
    ) {
      return;
    }
    if (columnIdSet.has(k)) {
      columnFilterMap.set(k, parseFilterValue(v));
      return;
    }
    query[k] = v;
  });

  const columnFilters: ColumnFiltersState = [];
  columnFilterMap.forEach((value, id) => {
    columnFilters.push({ id, value });
  });

  const sorting: SortingState = [];
  const sortBy = params.get("sortBy");
  if (sortBy) {
    const ascendingRaw = params.get("ascending");
    const ascending =
      ascendingRaw == null
        ? true
        : ["true", "1", "yes"].includes(String(ascendingRaw).toLowerCase());
    sorting.push({ id: sortBy, desc: !ascending });
  } else {
    const sort = params.get("sort");
    if (sort) {
      const [id, order] = sort.split(":");
      sorting.push({ id, desc: order === "desc" });
    }
  }

  const pageIndex = Number(params.get("page") || 1) - 1;
  const pageSize = Number(params.get("limit") || params.get("pageSize") || 10);

  return {
    query,
    table: { columnFilters, sorting, columnVisibility: {} },
    pagination: {
      pageIndex: Math.max(pageIndex, 0),
      pageSize,
    },
  };
}

function buildSearch(state: PageState) {
  const params = new URLSearchParams();

  Object.entries(state.query).forEach(([k, v]) => {
    if (v != null && v !== "") params.set(k, String(v));
  });

  if (state.table.columnFilters.length) {
    state.table.columnFilters.forEach((f) => {
      const value = (f as any).value;
      if (value == null || value === "") return;
      if (typeof value === "string") {
        params.set(f.id, value);
        return;
      }
      if (typeof value === "number" || typeof value === "boolean") {
        params.set(f.id, String(value));
        return;
      }
      try {
        params.set(f.id, JSON.stringify(value));
      } catch {
        params.set(f.id, String(value));
      }
    });
  }

  if (state.table.sorting[0]) {
    const s = state.table.sorting[0];
    params.set("sortBy", s.id);
    params.set("ascending", String(!s.desc));
  }

  params.set("page", String(state.pagination.pageIndex + 1));
  params.set("limit", String(state.pagination.pageSize));
  return params.toString();
}

export function buildSearchObject(
  state: PageState,
  withQuery = false,
): Record<string, any> {
  const params = new Map();

  if (withQuery) {
    Object.entries(state.query).forEach(([k, v]) => {
      if (v != null && v !== "") params.set(k, v);
    });
  }

  if (state.table.columnFilters.length) {
    state.table.columnFilters.forEach((f) => {
      const value = (f as any).value;
      if (value == null || value === "") return;
      if (typeof value === "string") {
        params.set(f.id, value);
        return;
      }
      if (typeof value === "number" || typeof value === "boolean") {
        params.set(f.id, value);
        return;
      }

      params.set(f.id, value);
    });
  }

  if (state.table.sorting[0]) {
    const s = state.table.sorting[0];
    params.set("sortBy", s.id);
    params.set("ascending", !s.desc);
  }

  params.set("page", state.pagination.pageIndex + 1);
  params.set("limit", state.pagination.pageSize);

  return Object.fromEntries(params);
}

function normalizeSearch(search: string): string {
  if (!search) return "";

  const params = new URLSearchParams(search);
  const entries = Array.from(params.entries());
  entries.sort(([aKey, aValue], [bKey, bValue]) => {
    if (aKey !== bKey) return aKey.localeCompare(bKey);
    return aValue.localeCompare(bValue);
  });

  const normalized = new URLSearchParams();
  entries.forEach(([k, v]) => normalized.append(k, v));
  return normalized.toString();
}

function loadFromCache(pageId: string): Partial<PageState> | null {
  const cached = lruCache.get(getStorageKey(pageId));
  if (cached == null || typeof cached !== "object") return null;
  return cached as Partial<PageState>;
}

type PersistFn = ((state: PageState) => void) & { cancel?: () => void };

const persistMap = new Map<string, PersistFn>();

function getPersist(pageId: string): PersistFn {
  const key = getStorageKey(pageId);
  const cached = persistMap.get(key);
  if (cached) return cached;

  const persist = throttle((state: PageState) => {
    lruCache.set(key, state);
  }, 500);
  persistMap.set(key, persist);
  return persist;
}

/* =========================
 * Store factory (with cache)
 * ========================= */

type PageStoreHook = UseBoundStore<StoreApi<PageStore>>;

const storeMap = new Map<string, PageStoreHook>();
const unsubscribeMap = new Map<string, () => void>();
const initOwnerMap = new Map<string, symbol>();

function mergeInitialQuery(
  state: PageState,
  initialQuery?: Record<string, any>,
): PageState {
  if (!initialQuery) return state;

  const nextQuery = { ...state.query };
  let changed = false;
  Object.entries(initialQuery).forEach(([k, v]) => {
    if (nextQuery[k] !== undefined) return;
    nextQuery[k] = v;
    changed = true;
  });

  if (!changed) return state;
  return {
    ...state,
    query: nextQuery,
  };
}

function createPageStore(
  pageId: string,
  search: string,
  columnIdSet: Set<string>,
  initialQuery?: Record<string, any>,
  cacheEnabled?: boolean,
): PageStoreHook {
  const cached = cacheEnabled ? loadFromCache(pageId) : null;

  if (cached && storeMap.has(pageId)) {
    return storeMap.get(pageId)!;
  }

  const persist = getPersist(pageId);
  let suppressPersist = false;

  const initial = mergeInitialQuery(
    parseFromUrl(search, columnIdSet) ??
      (() => {
        if (cached) {
          return {
            query: cached.query || {},
            table: cached.table || {
              columnFilters: [],
              sorting: [],
              columnVisibility: {},
            },
            pagination: {
              pageIndex: cached.pagination?.pageIndex ?? 0,
              pageSize: cached.pagination?.pageSize ?? 10,
            },
          };
        }
        return {
          query: {},
          table: { columnFilters: [], sorting: [], columnVisibility: {} },
          pagination: { pageIndex: 0, pageSize: 10 },
        };
      })(),
    initialQuery,
  );

  const persistedColumnVisibility = readColumnVisibility(pageId, columnIdSet);
  const initialColumnVisibility =
    Object.keys(persistedColumnVisibility).length > 0
      ? persistedColumnVisibility
      : sanitizeColumnVisibility(
          (initial.table as any).columnVisibility,
          columnIdSet,
        );

  const store = create<PageStore>((set) => ({
    ...initial,
    table: {
      ...initial.table,
      columnVisibility: initialColumnVisibility,
    },

    setQuery: (updater) =>
      set((s) => {
        const prevQuery = s.query;

        if (typeof updater === "function") {
          const nextQuery = updater(prevQuery);
          if (nextQuery === prevQuery) return s;
          return {
            query: nextQuery,
            pagination: { ...s.pagination, pageIndex: 0 },
          };
        }

        const patch = updater;
        let changed = false;
        const nextQuery: Record<string, any> = { ...prevQuery };
        Object.entries(patch).forEach(([k, v]) => {
          if (Object.is(prevQuery[k], v)) return;
          nextQuery[k] = v;
          changed = true;
        });
        if (!changed) return s;
        return {
          query: nextQuery,
          pagination: { ...s.pagination, pageIndex: 0 },
        };
      }),

    setColumnFilters: (updater) =>
      set((s) => {
        const prev = s.table.columnFilters;
        const next = typeof updater === "function" ? updater(prev) : updater;
        if (next === prev) return s;
        return {
          table: { ...s.table, columnFilters: next },
          pagination: { ...s.pagination, pageIndex: 0 },
        };
      }),

    setSorting: (updater) =>
      set((s) => {
        const prev = s.table.sorting;
        const next = typeof updater === "function" ? updater(prev) : updater;

        if (next === prev) return s;

        return {
          table: { ...s.table, sorting: next },
        };
      }),

    setColumnVisibility: (updater) =>
      set((s) => {
        const prev = s.table.columnVisibility;
        const computed =
          typeof updater === "function" ? updater(prev) : updater;
        const next = sanitizeColumnVisibility(computed, columnIdSet);
        if (isSameColumnVisibility(next, prev)) return s;

        writeColumnVisibility(pageId, next);
        return {
          table: { ...s.table, columnVisibility: next },
        };
      }),

    setPagination: (updater) =>
      set((s) => {
        const prev = s.pagination;
        const next = typeof updater === "function" ? updater(prev) : updater;
        if (next === prev) return s;
        if (
          next.pageIndex === prev.pageIndex &&
          next.pageSize === prev.pageSize
        )
          return s;
        return { pagination: next };
      }),

    reset: () => {
      suppressPersist = true;
      persist.cancel?.();
      lruCache.delete(getStorageKey(pageId));
      clearColumnVisibility(pageId);
      set({
        query: {},
        table: { columnFilters: [], sorting: [], columnVisibility: {} },
        pagination: { pageIndex: 0, pageSize: 10 },
      });
      suppressPersist = false;
    },
  }));

  store.subscribe((s) => {
    if (suppressPersist) return;
    persist({
      query: s.query,
      table: s.table,
      pagination: s.pagination,
    });
  });
  // unsubscribeMap.set(pageId, unsubscribe);
  storeMap.set(pageId, store);
  return store;
}

function getPageStore(pageId: string): PageStoreHook {
  const store = storeMap.get(pageId);
  if (!store) {
    throw new Error(
      `PageStore for "${pageId}" is not initialized. Call useInitPageStore first.`,
    );
  }
  return store;
}

export function useInitPageStore<T>({
  pageId,
  columns,
  initialQuery,
}: {
  pageId: string;
  columns: ColumnDef<T>[];
  initialQuery?: Record<string, any>;
}) {
  const isOwnerRef = useRef(false);
  const ownerTokenRef = useRef<symbol | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const searchString = searchParams.toString();
  const normalizedSearch = useMemo(
    () => normalizeSearch(searchString),
    [searchString],
  );
  const lastSyncedSearchRef = useRef<string>(normalizedSearch);
  const [cacheEnabled] = useState(() => shouldUseCache(searchString));

  const columnIdSet = useMemo(() => extractColumnIds(columns), [columns]);
  const store = useState<PageStoreHook>(() =>
    createPageStore(
      pageId,
      searchString,
      columnIdSet,
      initialQuery,
      cacheEnabled,
    ),
  )[0];

  const state = store();

  useEffect(() => {
    const token =
      ownerTokenRef.current ?? (ownerTokenRef.current = Symbol(pageId));
    if (!initOwnerMap.has(pageId)) {
      initOwnerMap.set(pageId, token);
      isOwnerRef.current = true;
    } else {
      isOwnerRef.current = initOwnerMap.get(pageId) === token;
    }
    return () => {
      if (initOwnerMap.get(pageId) === token) {
        initOwnerMap.delete(pageId);
      }
    };
  }, [pageId]);

  // useEffect(() => {
  //   if (!isOwnerRef.current) return;
  //   if (normalizedSearch === lastSyncedSearchRef.current) return;

  //   const next = parseFromUrl(searchString, columnIdSet);
  //   const currentSearch = normalizeSearch(buildSearch(store.getState()));
  //   console.log('next', searchString, next, currentSearch, normalizedSearch, store.getState());
  //   // if (next && currentSearch !== normalizedSearch) {
  //   //   store.setState(next);
  //   // }

  //   // lastSyncedSearchRef.current = normalizedSearch;
  // }, [columnIdSet, normalizedSearch, searchString, store]);

  // Store → URL
  useEffect(() => {
    if (!isOwnerRef.current) return;
    const nextSearch = buildSearch(state);
    const normalizedNextSearch = normalizeSearch(nextSearch);

    if (normalizedNextSearch === normalizedSearch) return;

    lastSyncedSearchRef.current = normalizedNextSearch;
    setSearchParams(nextSearch, { replace: true });
  }, [
    setSearchParams,
    normalizedSearch,
    state.query,
    state.table.columnFilters,
    state.table.sorting,
    state.pagination.pageIndex,
    state.pagination.pageSize,
  ]);

  const storageRef = useRef(false);
  useEffect(() => {
    if (!isOwnerRef.current) return;
    if (!storageRef.current) {
      storageRef.current = true;
      lruCache.set(getStorageKey(pageId), {
        query: state.query,
        table: state.table,
        pagination: state.pagination,
      });
    }
  }, [
    state.query,
    state.table.columnFilters,
    state.table.sorting,
    state.pagination.pageIndex,
    state.pagination.pageSize,
  ]);
}

export function usePageStoreState(pageId: string) {
  const store = getPageStore(pageId);
  return store((s) => ({
    query: s.query,
    table: s.table,
    pagination: s.pagination,
  }));
}

export function usePageStoreActions(pageId: string) {
  const store = getPageStore(pageId);
  return useMemo(() => {
    const s = store.getState();
    return {
      setQuery: s.setQuery,
      setColumnFilters: s.setColumnFilters,
      setSorting: s.setSorting,
      setColumnVisibility: s.setColumnVisibility,
      setPagination: s.setPagination,
      reset: s.reset,
    };
  }, [store]);
}

// export function usePageStoreTable(pageId: string) {
//   const store = getPageStore(pageId);
//   const state = store(s => ({
//     columnFilters: s.table.columnFilters,
//     sorting: s.table.sorting,
//     columnVisibility: s.table.columnVisibility,
//     pagination: s.pagination,
//   }));
//   return useMemo<PageTableController>(() => {
//     const s = store.getState();
//     return {
//       state,
//       setColumnFilters: s.setColumnFilters,
//       setSorting: s.setSorting,
//       setColumnVisibility: s.setColumnVisibility,
//       setPagination: s.setPagination,
//     };
//   }, [store, state]);
// }

export function usePageStore<T>({
  pageId,
  columns,
  initialQuery,
}: {
  pageId: string;
  columns: ColumnDef<T>[];
  initialQuery?: Record<string, any>;
}) {
  useInitPageStore({ pageId, columns, initialQuery });
  const state = usePageStoreState(pageId);
  const actions = usePageStoreActions(pageId);
  return {
    ...state,
    ...actions,
  };
}

export function useSyncExternalQuery<T>(
  key: string,
  value: T,
  setQuery: PageStore["setQuery"],
) {
  const lastRef = useRef(value);

  useEffect(() => {
    if (Object.is(lastRef.current, value)) return;
    lastRef.current = value;
    setQuery({ [key]: value });
  }, [key, value, setQuery]);
}

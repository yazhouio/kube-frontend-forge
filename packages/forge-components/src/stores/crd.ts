import useSWR from "swr";
import { PathParams, Store } from "./interfaces";
import { get, set } from "es-toolkit/compat";
import qs from "qs";
import { getUrlHof } from "./utils";
import { PublicConfiguration } from "swr/_internal";

export const fetchHandler = {
  get: async (url: string, query?: Record<string, any>, kapi?: boolean) => {
    const queryString = query
      ? qs.stringify(query ?? {}, { skipNulls: true })
      : "";
    const requestUrl = queryString
      ? `${url}${url.includes("?") ? "&" : "?"}${queryString}`
      : url;
    const resp = await fetch(requestUrl, {
      method: "GET",
      headers: {
        "Content-Type": "application/json",
      },
    });
    return parseResponseData(resp, kapi);
  },
  post: (url: string, body: any) => {
    return fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
  },
  put: async (url: string, body: any, noGetDetail = false) => {
    if (!noGetDetail) {
      const data = await fetchHandler.get(url);
      const resourceVersion = get(data, "metadata.resourceVersion");
      if (resourceVersion) {
        set(body, "metadata.resourceVersion", resourceVersion);
      }
    }
    return fetch(url, {
      method: "PUT",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
  },
  delete: (url: string) => {
    return fetch(url, {
      method: "DELETE",
      headers: {
        "Content-Type": "application/json",
      },
    });
  },
};

const parseResponseBody = async (resp: Response) => {
  const text = await resp.text();
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
};

const parseResponseData = async (resp: Response, kapi?: boolean) => {
  const data = await parseResponseBody(resp);
  if (kapi) {
    return {
      data: get(data, "items"),
      total: get(data, "totalItems") || get(data, "metadata.continue"),
      status: resp.status,
      ok: resp.ok,
    };
  }
  const items = get(data, "items");
  const remainingItemCount = get(data, "metadata.remainingItemCount", 0);
  const total = items?.length + remainingItemCount;
  return {
    data: items,
    total,
    status: resp.status,
    ok: resp.ok,
  };
};

export const getCrdStore = (store: Store) => {
  return function useStore(
    {
      params,
      query,
      k8sVersion,
    }: {
      params?: PathParams;
      query?: Record<string, any>;
      k8sVersion?: string;
    },
    options?: Partial<PublicConfiguration> & { enabled?: boolean },
  ) {
    const { enabled = true, ...rest } = options || {};
    const url = getUrlHof(store, k8sVersion)(params || {});
    const key = [url, query];

    const swr = useSWR(
      enabled ? key : null,
      () => fetchHandler.get(url, query, store.kapi),
      rest,
    );
    const create = async (params: PathParams, body: any) => {
      const createUrl = getUrlHof(store, k8sVersion)(params);
      const res = await fetchHandler.post(createUrl, body);
      swr.mutate();
      return res;
    };
    const update = async (
      params: PathParams,
      body: any,
      noGetDetail = false,
    ) => {
      const updateUrl = getUrlHof(store, k8sVersion)(params);
      const res = await fetchHandler.put(updateUrl, body, noGetDetail);
      swr.mutate();
      return res;
    };
    const del = async (params: PathParams, resolve = true) => {
      const deleteUrl = getUrlHof(store, k8sVersion)(params);
      const res = await fetchHandler.delete(deleteUrl);
      if (resolve) {
        swr.mutate();
      } else {
        swr.mutate(null, false);
      }
      return res;
    };
    const batchDelete = async (resources: PathParams[]) => {
      const promises = resources.map((item) => {
        const deleteUrl = getUrlHof(store, k8sVersion)(item);
        return fetchHandler.delete(deleteUrl);
      });
      const res = await Promise.allSettled(promises);
      swr.mutate();
      return res;
    };
    return {
      ...swr,
      create,
      update,
      delete: del,
      batchDelete,
    };
  };
};

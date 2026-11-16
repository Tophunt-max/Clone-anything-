import { createContext, useContext } from "react";
import type { ApiClient } from "@forge/api-client";

export const ApiContext = createContext<ApiClient | null>(null);

export function useApi(): ApiClient {
  const api = useContext(ApiContext);
  if (!api) throw new Error("useApi needs an ApiContext.Provider");
  return api;
}

import { useCallback, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

const MAX_ENTRIES = 48;

/**
 * 记忆详情里展开文件时的内容缓存 + 可选预取，减少重复 IPC 与「加载中」闪烁。
 * 当路径不再出现在当前列表中时自动剔除缓存项。
 */
export function useDetailMemoryFileCache(allowedPaths: string[]) {
  const fingerprint = useMemo(
    () => [...new Set(allowedPaths)].sort().join("\0"),
    [allowedPaths],
  );
  const allowedRef = useRef<Set<string>>(new Set());
  const cacheRef = useRef<Map<string, string>>(new Map());
  const inFlightRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    allowedRef.current = new Set(allowedPaths);
  }, [fingerprint, allowedPaths]);

  useEffect(() => {
    const allow = allowedRef.current;
    const c = cacheRef.current;
    for (const k of [...c.keys()]) {
      if (!allow.has(k)) c.delete(k);
    }
  }, [fingerprint]);

  const touch = useCallback((path: string, content: string) => {
    if (!allowedRef.current.has(path)) return;
    const m = cacheRef.current;
    m.delete(path);
    m.set(path, content);
    while (m.size > MAX_ENTRIES) {
      const first = m.keys().next().value;
      if (first === undefined) break;
      m.delete(first);
    }
  }, []);

  const getCached = useCallback((path: string) => cacheRef.current.get(path), []);

  const prefetchPath = useCallback(
    (path: string) => {
      if (!allowedRef.current.has(path)) return;
      if (cacheRef.current.has(path)) return;
      if (inFlightRef.current.has(path)) return;
      inFlightRef.current.add(path);
      void invoke<string>("skilllite_read_memory_file", { relativePath: path })
        .then((content) => {
          touch(path, content);
        })
        .catch(() => {})
        .finally(() => {
          inFlightRef.current.delete(path);
        });
    },
    [touch],
  );

  return { getCached, touch, prefetchPath };
}

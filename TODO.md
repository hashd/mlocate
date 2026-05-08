# TODO

## Database Size Optimization

File directory (raw paths + metadata) dominates ~90% of DB size. Paths are already sorted, making them amenable to compression.

| Approach | DB Size | Indexing Time | Search Time |
|---|---|---|---|
| Current (raw paths) | 54 MB | 120s | O(1) per result, ~0 µs |
| Front-coding + checkpoints | 22-25 MB | ~115s | ~10-20 ms per 1K results |
| **Zstd blocks (4 KB)** | **15-21 MB** | ~118s | ~1-2 ms per 1K results |
| Prefix table + suffix | 25-30 MB | ~125s | O(1) per result, ~0 µs |

**Recommended:** Zstd block compression with a small LRU block cache.
